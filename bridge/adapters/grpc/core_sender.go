// Package grpcadapter provides the gRPC adapter for sending messages to Rust core.
package grpcadapter

import (
	"context"
	"fmt"
	"time"

	"github.com/rs/zerolog"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"

	"pharma-bridge/domain"
	"pharma-bridge/ports"
	pb "pharma-bridge/proto"
)

// CoreSender implements ports.MessageSink for gRPC communication with Rust core.
type CoreSender struct {
	client  pb.PharmaCoreClient
	conn    *grpc.ClientConn
	circuit ports.CircuitBreaker
	logger  zerolog.Logger
}

// CoreSenderConfig holds configuration for the gRPC sender.
type CoreSenderConfig struct {
	Address        string
	ConnectTimeout time.Duration
}

// NewCoreSender creates a new gRPC sender.
func NewCoreSender(cfg CoreSenderConfig, circuit ports.CircuitBreaker, logger zerolog.Logger) (*CoreSender, error) {
	conn, err := grpc.NewClient(
		cfg.Address,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
	)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to gRPC: %w", err)
	}

	client := pb.NewPharmaCoreClient(conn)

	ctx, cancel := context.WithTimeout(context.Background(), cfg.ConnectTimeout)
	defer cancel()

	resp, err := client.HealthCheck(ctx, &pb.HealthRequest{})
	if err != nil {
		logger.Warn().Err(err).Msg("Could not reach Rust core (will retry on messages)")
	} else {
		logger.Info().
			Bool("healthy", resp.Healthy).
			Str("version", resp.Version).
			Int64("uptime", resp.UptimeSeconds).
			Msg("✅ Connected to Rust core")
	}

	return &CoreSender{
		client:  client,
		conn:    conn,
		circuit: circuit,
		logger:  logger.With().Str("component", "grpc_sender").Logger(),
	}, nil
}

// Send forwards a message to the Rust core via gRPC.
func (s *CoreSender) Send(ctx context.Context, msg domain.Message) error {
	if !s.circuit.Allow() {
		return fmt.Errorf("circuit breaker is open")
	}

	traceID := domain.NewTraceID(msg.ID, time.Now().UnixNano())

	md := metadata.Pairs("x-request-id", traceID.String())
	ctx = metadata.NewOutgoingContext(ctx, md)

	protoMsg := &pb.RawMessage{
		Id:          msg.ID.String(),
		ExternalId:  msg.ExternalID.String(),
		GroupJid:    msg.GroupJID.String(),
		GroupName:   msg.GroupName,
		SenderJid:   msg.SenderJID.String(),
		SenderPhone: msg.SenderPhone.String(),
		SenderName:  msg.SenderName,
		Content:     msg.Content,
		Timestamp:   msg.Timestamp.Int64(),
	}

	s.logger.Info().
		Str("trace_id", traceID.String()).
		Str("id", msg.ID.String()).
		Msg("📤 Forwarding message to Rust core")

	resp, err := s.client.ProcessMessage(ctx, protoMsg)
	if err != nil {
		s.circuit.RecordFailure()
		return err
	}

	s.circuit.RecordSuccess()

	s.logger.Info().
		Str("trace_id", traceID.String()).
		Bool("success", resp.Success).
		Str("message_id", resp.MessageId).
		Msg("📤 Message forwarded to Rust core")

	return nil
}

// Close releases resources.
func (s *CoreSender) Close() error {
	if s.conn != nil {
		// Ignore "connection is closing" errors during shutdown
		_ = s.conn.Close()
	}
	return nil
}

// GetMonitoredGroups fetches the list of monitored groups from the Rust core.
func (s *CoreSender) GetMonitoredGroups(ctx context.Context) ([]domain.JID, error) {
	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	resp, err := s.client.GetMonitoredGroups(ctx, &pb.MonitoredGroupsRequest{})
	if err != nil {
		return nil, err
	}

	jids := make([]domain.JID, len(resp.Jids))
	for i, jid := range resp.Jids {
		jids[i] = domain.JID(jid)
	}
	return jids, nil
}

// SyncGroups sends groups from WhatsApp to the Rust core for storage.
func (s *CoreSender) SyncGroups(ctx context.Context, groups []domain.GroupInfo) (added, updated int32, err error) {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()

	protoGroups := make([]*pb.GroupInfo, len(groups))
	for i, g := range groups {
		protoGroups[i] = &pb.GroupInfo{
			Jid:         g.JID.String(),
			Name:        g.Name,
			Description: g.Description,
		}
	}

	resp, err := s.client.SyncGroups(ctx, &pb.SyncGroupsRequest{
		Groups: protoGroups,
	})
	if err != nil {
		s.logger.Error().Err(err).Msg("Failed to sync groups to Core")
		return 0, 0, err
	}

	if !resp.Success {
		return 0, 0, fmt.Errorf("sync failed: %s", resp.Error)
	}

	s.logger.Info().
		Int32("added", resp.Added).
		Int32("updated", resp.Updated).
		Msg("✅ Groups synced to Core")

	return resp.Added, resp.Updated, nil
}

var _ ports.MessageSink = (*CoreSender)(nil)
var _ ports.GroupRepository = (*CoreSender)(nil)
var _ ports.GroupSyncer = (*CoreSender)(nil)
