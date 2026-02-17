// Package main is the composition root for the bridge using Uber FX.
package main

import (
	"context"
	"net"
	"net/http"
	"os"

	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
	"go.uber.org/fx"
	"google.golang.org/grpc"

	grpcadapter "pharma-bridge/adapters/grpc"
	qradapter "pharma-bridge/adapters/qr"
	resilienceadapter "pharma-bridge/adapters/resilience"
	"pharma-bridge/adapters/whatsapp"
	"pharma-bridge/app"
	"pharma-bridge/cache"
	"pharma-bridge/deduplicator"
	"pharma-bridge/domain"
	"pharma-bridge/infra/config"
	infrahttp "pharma-bridge/infra/http"
	"pharma-bridge/ports"
	"pharma-bridge/qr"
	"pharma-bridge/resilience"
)

func main() {
	zerolog.TimeFieldFormat = zerolog.TimeFormatUnix
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})

	// Load config early to get log level
	cfg, err := config.Load()
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to load config")
	}

	// Set log level from config
	level, err := zerolog.ParseLevel(cfg.LogLevel)
	if err != nil {
		level = zerolog.InfoLevel // Default to info if invalid
		log.Warn().Str("log_level", cfg.LogLevel).Msg("Invalid log level, using 'info'")
	}
	zerolog.SetGlobalLevel(level)
	log.Debug().Str("log_level", level.String()).Msg("Log level set from config")

	log.Info().Str("version", domain.CurrentVersion.String()).Msg("🌉 PharmaBroker WhatsApp Bridge")

	fxApp := fx.New(
		fx.NopLogger,

		fx.Provide(func() (*config.Config, error) { return cfg, nil }),
		fx.Provide(func() zerolog.Logger { return log.Logger }),

		// Resilience
		fx.Provide(provideCircuitBreaker),
		fx.Provide(provideRateLimiter),
		fx.Provide(provideGroupCache),
		fx.Provide(provideDeduplicator),

		// Adapters
		fx.Provide(provideQRHandler),
		fx.Provide(provideCoreSender),
		fx.Provide(provideBridgeServer),
		fx.Provide(provideRetrySender),
		fx.Provide(provideWhatsAppClient),

		// App
		fx.Provide(provideBridge),

		// HTTP Server
		fx.Provide(provideHTTPServer),

		// Lifecycle
		fx.Invoke(registerRoutes),
		fx.Invoke(startHTTPServer),
		fx.Invoke(startBridgeGRPCServer),
		fx.Invoke(startBridge),
	)

	fxApp.Run()
}

func provideCircuitBreaker(cfg *config.Config) ports.CircuitBreaker {
	cb := resilience.NewCircuitBreaker(cfg.Resilience.CircuitBreaker.MaxFailures, cfg.Resilience.CircuitBreaker.Timeout)
	cb.SetOnStateChange(func(s resilience.State) {
		log.Warn().Int("state", int(s)).Msg("Circuit breaker state changed")
	})
	return cb
}

func provideRateLimiter(cfg *config.Config) ports.RateLimiter {
	return resilience.NewRateLimiter(resilience.RateLimiterConfig{
		RatePerMinute: cfg.RateLimit.PerMinute,
		BurstSize:     cfg.RateLimit.BurstSize,
		Enabled:       cfg.RateLimit.Enabled,
	})
}

func provideGroupCache(cfg *config.Config) ports.GroupCache {
	return cache.NewGroupCache(cfg.GroupSync.Interval)
}

func provideDeduplicator(lc fx.Lifecycle, cfg *config.Config, logger zerolog.Logger) ports.Deduplicator {
	ctx, cancel := context.WithCancel(context.Background())
	dedup := deduplicator.New(ctx, deduplicator.Config{
		Window:          cfg.Dedup.Window,
		CacheSize:       cfg.Dedup.CacheSize,
		CacheTTL:        cfg.Dedup.CacheTTL,
		CleanupInterval: cfg.Dedup.CleanupInterval,
	}, logger)

	lc.Append(fx.Hook{
		OnStop: func(context.Context) error {
			cancel()
			dedup.Close()
			return nil
		},
	})

	return dedup
}

func provideQRHandler(cfg *config.Config, logger zerolog.Logger) *qradapter.HandlerAdapter {
	return qradapter.NewHandlerAdapter(qr.Config{
		RenderTerminal: cfg.WhatsApp.QRTerminal,
		QRTimeout:      cfg.WhatsApp.QRTimeout,
		MaxRetries:     cfg.WhatsApp.QRRetries,
	}, logger)
}

func provideCoreSender(
	lc fx.Lifecycle,
	cfg *config.Config,
	circuit ports.CircuitBreaker,
	logger zerolog.Logger,
) (*grpcadapter.CoreSender, error) {
	sender, err := grpcadapter.NewCoreSender(
		grpcadapter.CoreSenderConfig{
			Address:        cfg.GRPC.CoreAddr,
			ConnectTimeout: cfg.GRPC.ConnectTimeout,
		},
		circuit,
		logger,
	)
	if err != nil {
		return nil, err
	}

	lc.Append(fx.Hook{
		OnStop: func(context.Context) error { return sender.Close() },
	})

	return sender, nil
}

func provideBridgeServer(
	waClient *whatsapp.Client,
	cfg *config.Config,
	logger zerolog.Logger,
) *grpcadapter.BridgeServer {
	return grpcadapter.NewBridgeServer(waClient, cfg.WhatsApp.OperatorJID, logger)
}

func provideRetrySender(
	lc fx.Lifecycle,
	coreSender *grpcadapter.CoreSender,
	cfg *config.Config,
	logger zerolog.Logger,
) ports.MessageSink {
	retryCfg := resilienceadapter.RetrySenderConfig{
		MaxSize:       cfg.Resilience.RetryBuffer.MaxSize,
		FlushInterval: cfg.Resilience.RetryBuffer.FlushInterval,
	}
	sender := resilienceadapter.NewRetrySender(coreSender, retryCfg, logger)

	ctx, cancel := context.WithCancel(context.Background())
	sender.Start(ctx, retryCfg)

	lc.Append(fx.Hook{
		OnStop: func(context.Context) error {
			cancel()
			return sender.Close()
		},
	})

	return sender
}

func provideWhatsAppClient(
	lc fx.Lifecycle,
	cfg *config.Config,
	qrHandler *qradapter.HandlerAdapter,
	logger zerolog.Logger,
) (*whatsapp.Client, error) {
	client, err := whatsapp.NewClient(context.Background(), whatsapp.ClientConfig{
		StorePath:    cfg.WhatsApp.StorePath,
		QRMaxRetries: cfg.WhatsApp.QRRetries,
		QRTimeout:    cfg.WhatsApp.QRTimeout,
	}, qrHandler, logger)
	if err != nil {
		return nil, err
	}

	lc.Append(fx.Hook{
		OnStop: func(context.Context) error {
			client.Disconnect()
			return nil
		},
	})

	return client, nil
}

func provideBridge(
	waClient *whatsapp.Client,
	sink ports.MessageSink,
	groupCache ports.GroupCache,
	coreSender *grpcadapter.CoreSender,
	dedup ports.Deduplicator,
	rateLimiter ports.RateLimiter,
	cfg *config.Config,
	logger zerolog.Logger,
) *app.Bridge {
	return app.NewBridge(app.BridgeParams{
		Source:        waClient,
		Sink:          sink,
		GroupCache:    groupCache,
		GroupRepo:     coreSender,
		GroupSyncer:   coreSender,
		GroupProvider: waClient,
		Dedup:         dedup,
		RateLimiter:   rateLimiter,
		Logger:        logger,
		Config: app.BridgeConfig{
			SkipOwnMessages:   cfg.Processing.SkipOwnMessages,
			WorkerCount:       cfg.Processing.WorkerCount,
			WorkerQueueSize:   cfg.Processing.WorkerQueueSize,
			GroupSyncInterval: cfg.GroupSync.Interval,
		},
	})
}

func provideHTTPServer(cfg *config.Config, logger zerolog.Logger) *infrahttp.Server {
	return infrahttp.NewServer(infrahttp.ServerConfig{
		Port: cfg.HTTP.Port,
		Mode: cfg.HTTP.Mode,
	}, logger)
}

// HealthDeps holds dependencies for health endpoint.
type HealthDeps struct {
	Bridge      *app.Bridge
	RateLimiter ports.RateLimiter
	Dedup       ports.Deduplicator
	Sink        ports.MessageSink
	Circuit     ports.CircuitBreaker
}

func registerRoutes(
	server *infrahttp.Server,
	qrHandler *qradapter.HandlerAdapter,
	bridge *app.Bridge,
	rateLimiter ports.RateLimiter,
	dedup ports.Deduplicator,
	sink ports.MessageSink,
	circuit ports.CircuitBreaker,
) {
	engine := server.Engine()

	// Health endpoint
	engine.GET("/health", healthHandler(HealthDeps{
		Bridge:      bridge,
		RateLimiter: rateLimiter,
		Dedup:       dedup,
		Sink:        sink,
		Circuit:     circuit,
	}))

	// Sync groups endpoint
	engine.POST("/sync-groups", syncGroupsHandler(bridge))

	// QR endpoints
	qrHandler.RegisterRoutes(engine.Group("/qr"))
}

func syncGroupsHandler(bridge *app.Bridge) gin.HandlerFunc {
	return func(c *gin.Context) {
		if !bridge.IsConnected() {
			c.JSON(http.StatusServiceUnavailable, gin.H{
				"success": false,
				"error":   "WhatsApp not connected",
			})
			return
		}

		// Trigger group sync in background
		go bridge.TriggerGroupSync(context.Background())

		c.JSON(http.StatusOK, gin.H{
			"success": true,
			"message": "Group sync triggered",
		})
	}
}

func healthHandler(deps HealthDeps) gin.HandlerFunc {
	return func(c *gin.Context) {
		connected := deps.Bridge.IsConnected()

		resp := infrahttp.NewHealthResponse()
		resp.WhatsAppConnected = connected
		resp.MessagesForwarded = deps.Bridge.MessagesForwarded()

		if connected {
			resp.Status = "healthy"
		} else {
			resp.Status = "whatsapp_disconnected"
		}

		// Circuit breaker state
		resp.CircuitBreaker = "closed"
		if cb, ok := deps.Circuit.(*resilience.CircuitBreaker); ok && cb != nil {
			switch cb.State() {
			case resilience.StateOpen:
				resp.CircuitBreaker = "open"
			case resilience.StateHalfOpen:
				resp.CircuitBreaker = "half_open"
			}
		}

		// Optional stats
		if rs, ok := deps.Sink.(*resilienceadapter.RetrySender); ok && rs != nil {
			resp.RetryBufferSize = rs.Size()
		}
		if dd, ok := deps.Dedup.(*deduplicator.Deduplicator); ok && dd != nil {
			resp.DeduplicatorStats = dd.Stats()
		}
		if rl, ok := deps.RateLimiter.(*resilience.RateLimiter); ok && rl != nil {
			resp.RateLimiterStats = rl.GetStats()
		}

		status := http.StatusOK
		if resp.Status != "healthy" {
			status = http.StatusServiceUnavailable
		}

		c.JSON(status, resp)
	}
}

func startBridge(lc fx.Lifecycle, bridge *app.Bridge, waClient *whatsapp.Client, logger zerolog.Logger) {
	lc.Append(fx.Hook{
		OnStart: func(ctx context.Context) error {
			// Start WhatsApp connection in background to not block HTTP server
			go func() {
				if err := waClient.Connect(context.Background()); err != nil {
					logger.Error().Err(err).Msg("Failed to connect WhatsApp")
					return
				}

				if err := bridge.Run(context.Background()); err != nil {
					logger.Error().Err(err).Msg("Bridge stopped with error")
				}
			}()

			return nil
		},
		OnStop: func(context.Context) error {
			bridge.Stop()
			return nil
		},
	})
}

func startHTTPServer(lc fx.Lifecycle, server *infrahttp.Server, cfg *config.Config, logger zerolog.Logger) {
	lc.Append(fx.Hook{
		OnStart: func(context.Context) error {
			server.Start()
			logger.Info().Str("url", "http://localhost:"+cfg.HTTP.Port+"/qr").Msg("📱 QR code available at")
			return nil
		},
		OnStop: func(ctx context.Context) error {
			return server.Shutdown(ctx)
		},
	})
}

func startBridgeGRPCServer(lc fx.Lifecycle, bridgeServer *grpcadapter.BridgeServer, cfg *config.Config, logger zerolog.Logger) {
	server := grpc.NewServer()
	bridgeServer.Register(server)

	lc.Append(fx.Hook{
		OnStart: func(ctx context.Context) error {
			lis, err := net.Listen("tcp", ":"+cfg.GRPC.Port)
			if err != nil {
				return err
			}
			go func() {
				logger.Info().Str("port", cfg.GRPC.Port).Msg("📡 Bridge gRPC server listening")
				if err := server.Serve(lis); err != nil {
					logger.Error().Err(err).Msg("Bridge gRPC server failed")
				}
			}()
			return nil
		},
		OnStop: func(ctx context.Context) error {
			server.GracefulStop()
			return nil
		},
	})
}
