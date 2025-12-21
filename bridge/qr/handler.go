// Package qr provides QR code handling for WhatsApp pairing.
package qr

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"image/png"
	"net/http"
	"os"
	"sync"
	"sync/atomic"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/gorilla/websocket"
	"github.com/mdp/qrterminal/v3"
	"github.com/rs/zerolog"
	"github.com/skip2/go-qrcode"
)

// State represents the current QR/pairing state.
type State string

const (
	StateWaiting  State = "waiting"
	StateReady    State = "ready"
	StateScanning State = "scanning"
	StatePaired   State = "paired"
	StateTimeout  State = "timeout"
	StateError    State = "error"
)

// Update represents a QR code update event.
type Update struct {
	State     State     `json:"state"`
	Code      string    `json:"code,omitempty"`
	Image     string    `json:"image,omitempty"`
	ExpiresAt time.Time `json:"expires_at"`
	Error     string    `json:"error,omitempty"`
	Attempt   int       `json:"attempt,omitempty"`
	MaxRetry  int       `json:"max_retry,omitempty"`
}

// Config holds QR handler configuration.
type Config struct {
	RenderTerminal bool
	QRTimeout      time.Duration
	MaxRetries     int
}

// Handler manages QR code display and distribution.
type Handler struct {
	logger    zerolog.Logger
	mu        sync.RWMutex
	state     State
	qrCode    string
	qrImage   string
	expiresAt time.Time
	error     string
	attempt   int
	maxRetry  int
	clients   map[*websocket.Conn]bool
	clientsMu sync.RWMutex
	updates   chan Update
	closed    atomic.Bool
}

// New creates a new QR handler.
func New(cfg Config, logger zerolog.Logger) *Handler {
	h := &Handler{
		logger:  logger.With().Str("component", "qr").Logger(),
		state:   StateWaiting,
		clients: make(map[*websocket.Conn]bool),
		updates: make(chan Update, 10),
	}
	go h.broadcastWorker()
	return h
}

// HandleQRCode processes a new QR code.
func (h *Handler) HandleQRCode(code string, cfg Config) {
	image := generateQRBase64(code)

	h.mu.Lock()
	h.qrCode = code
	h.qrImage = image
	h.state = StateReady
	h.expiresAt = time.Now().Add(cfg.QRTimeout)
	h.error = ""
	h.attempt++
	h.maxRetry = cfg.MaxRetries
	attempt, maxRetry := h.attempt, h.maxRetry
	expiresAt := h.expiresAt
	h.mu.Unlock()

	h.logger.Info().
		Int("attempt", attempt).
		Int("max_retry", maxRetry).
		Msg("📱 QR code ready for scanning")

	if cfg.RenderTerminal {
		h.renderTerminal(code, attempt, maxRetry)
	}

	if !h.closed.Load() {
		select {
		case h.updates <- Update{
			State:     StateReady,
			Code:      code,
			Image:     image,
			ExpiresAt: expiresAt,
			Attempt:   attempt,
			MaxRetry:  maxRetry,
		}:
		default:
		}
	}
}

func generateQRBase64(code string) string {
	qr, err := qrcode.New(code, qrcode.Medium)
	if err != nil {
		return ""
	}
	var buf bytes.Buffer
	if err := png.Encode(&buf, qr.Image(256)); err != nil {
		return ""
	}
	return base64.StdEncoding.EncodeToString(buf.Bytes())
}

// HandleEvent processes QR channel events.
func (h *Handler) HandleEvent(event string, cfg Config) {
	var update Update

	h.mu.Lock()
	switch event {
	case "success":
		h.state = StatePaired
		h.qrCode, h.qrImage = "", ""
		h.logger.Info().Msg("✅ WhatsApp paired successfully")
		update = Update{State: StatePaired}
	case "timeout":
		h.state = StateTimeout
		h.qrCode, h.qrImage = "", ""
		h.logger.Warn().Msg("⏰ QR code expired")
		update = Update{State: StateTimeout}
	}
	h.mu.Unlock()

	if update.State != "" && !h.closed.Load() {
		select {
		case h.updates <- update:
		default:
		}
	}
}

// HandleError records an error state.
func (h *Handler) HandleError(err error) {
	h.mu.Lock()
	h.state = StateError
	h.error = err.Error()
	h.qrCode, h.qrImage = "", ""
	errMsg := h.error
	h.mu.Unlock()

	h.logger.Error().Err(err).Msg("QR pairing error")

	if !h.closed.Load() {
		select {
		case h.updates <- Update{State: StateError, Error: errMsg}:
		default:
		}
	}
}

// SetPaired marks the handler as paired.
func (h *Handler) SetPaired() {
	h.mu.Lock()
	h.state = StatePaired
	h.qrCode, h.qrImage = "", ""
	h.mu.Unlock()
}

// IsPaired returns true if paired.
func (h *Handler) IsPaired() bool {
	h.mu.RLock()
	defer h.mu.RUnlock()
	return h.state == StatePaired
}

// GetState returns the current QR state.
func (h *Handler) GetState() Update {
	h.mu.RLock()
	defer h.mu.RUnlock()
	return Update{
		State:     h.state,
		Code:      h.qrCode,
		Image:     h.qrImage,
		ExpiresAt: h.expiresAt,
		Error:     h.error,
		Attempt:   h.attempt,
		MaxRetry:  h.maxRetry,
	}
}

func (h *Handler) renderTerminal(code string, attempt, maxRetry int) {
	retryInfo := fmt.Sprintf("  📊 Attempt %d", attempt)
	if maxRetry > 0 {
		retryInfo = fmt.Sprintf("  📊 Attempt %d of %d", attempt, maxRetry)
	}

	println("\n═══════════════════════════════════════════════════════")
	println("  📱 Scan this QR code with WhatsApp to link device")
	println(retryInfo)
	println("═══════════════════════════════════════════════════════\n")
	qrterminal.GenerateHalfBlock(code, qrterminal.L, os.Stdout)
	println("\n═══════════════════════════════════════════════════════")
	println("  ⏰ QR code expires in 60 seconds")
	println("  🌐 Or visit http://localhost:5050/qr for web view")
	println("═══════════════════════════════════════════════════════\n")
}

// --- Gin Handlers ---

// RegisterRoutes registers QR routes on a Gin router group.
func (h *Handler) RegisterRoutes(rg *gin.RouterGroup) {
	rg.GET("", h.HTMLHandler)
	rg.GET("/json", h.JSONHandler)
	rg.GET("/ws", h.WebSocketHandler)
}

// JSONHandler returns the QR code as JSON.
func (h *Handler) JSONHandler(c *gin.Context) {
	state := h.GetState()
	c.Header("Cache-Control", "no-cache, no-store, must-revalidate")

	status := http.StatusAccepted
	switch state.State {
	case StateReady, StatePaired:
		status = http.StatusOK
	case StateTimeout:
		status = http.StatusGone
	case StateError:
		status = http.StatusInternalServerError
	}

	c.JSON(status, state)
}

// HTMLHandler returns the QR code HTML page.
func (h *Handler) HTMLHandler(c *gin.Context) {
	state := h.GetState()
	c.Header("Cache-Control", "no-cache, no-store, must-revalidate")

	stateJSON, _ := json.Marshal(state)
	html := fmt.Sprintf(qrHTMLTemplate, string(stateJSON))
	c.Data(http.StatusOK, "text/html; charset=utf-8", []byte(html))
}

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool { return true },
}

// WebSocketHandler handles WebSocket connections.
func (h *Handler) WebSocketHandler(c *gin.Context) {
	conn, err := upgrader.Upgrade(c.Writer, c.Request, nil)
	if err != nil {
		h.logger.Error().Err(err).Msg("WebSocket upgrade failed")
		return
	}

	h.clientsMu.Lock()
	h.clients[conn] = true
	h.clientsMu.Unlock()

	// Send current state
	data, _ := json.Marshal(h.GetState())
	conn.WriteMessage(websocket.TextMessage, data)

	defer func() {
		h.clientsMu.Lock()
		delete(h.clients, conn)
		h.clientsMu.Unlock()
		conn.Close()
	}()

	for {
		if _, _, err := conn.ReadMessage(); err != nil {
			break
		}
	}
}

func (h *Handler) broadcastWorker() {
	for update := range h.updates {
		data, _ := json.Marshal(update)
		h.clientsMu.RLock()
		for conn := range h.clients {
			conn.WriteMessage(websocket.TextMessage, data)
		}
		h.clientsMu.RUnlock()
	}
}

// Close shuts down the handler.
func (h *Handler) Close() {
	if h.closed.Swap(true) {
		return // Already closed
	}
	close(h.updates)
	h.clientsMu.Lock()
	for conn := range h.clients {
		conn.Close()
	}
	h.clients = nil
	h.clientsMu.Unlock()
}

const qrHTMLTemplate = `<!DOCTYPE html>
<html>
<head>
    <title>WhatsApp QR Code</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex; justify-content: center; align-items: center; 
            min-height: 100vh; margin: 0;
            background: linear-gradient(135deg, #667eea 0%%, #764ba2 100%%); }
        .container { background: white; padding: 40px; border-radius: 20px;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3); text-align: center; max-width: 400px; }
        h1 { color: #333; margin-bottom: 10px; }
        p { color: #666; margin-bottom: 20px; }
        #qrcode { margin: 20px auto; min-height: 260px; 
            display: flex; align-items: center; justify-content: center; }
        #qrcode img { border-radius: 10px; box-shadow: 0 4px 12px rgba(0,0,0,0.1); }
        .status { padding: 10px 20px; border-radius: 20px; 
            display: inline-block; margin-top: 20px; font-weight: 500; }
        .status.ready { background: #e3f2fd; color: #1976d2; }
        .status.paired { background: #e8f5e9; color: #388e3c; }
        .status.timeout { background: #fff3e0; color: #f57c00; }
        .status.error { background: #ffebee; color: #d32f2f; }
        .status.waiting { background: #f5f5f5; color: #757575; }
        .timer { font-size: 14px; color: #999; margin-top: 10px; }
        .icon { font-size: 80px; }
        .loading { font-size: 40px; animation: pulse 1.5s infinite; }
        @keyframes pulse { 0%%, 100%% { opacity: 1; } 50%% { opacity: 0.5; } }
    </style>
</head>
<body>
    <div class="container">
        <h1>📱 WhatsApp Pairing</h1>
        <p>Scan this QR code with WhatsApp</p>
        <div id="qrcode"><div class="loading">⏳</div></div>
        <div id="status" class="status waiting">Connecting...</div>
        <div id="timer" class="timer"></div>
    </div>
    <script>
        var initialState = %s;
        var statusEl = document.getElementById('status');
        var timerEl = document.getElementById('timer');
        var qrcodeEl = document.getElementById('qrcode');
        var timerInterval = null;
        
        function updateUI(data) {
            statusEl.className = 'status ' + data.state;
            if (timerInterval) { clearInterval(timerInterval); timerInterval = null; }
            var attemptInfo = data.attempt > 0 ? (data.max_retry > 0 ? 
                ' (Attempt ' + data.attempt + '/' + data.max_retry + ')' : 
                ' (Attempt ' + data.attempt + ')') : '';
            
            if (data.state === 'ready') {
                statusEl.textContent = '⏳ Waiting for scan...' + attemptInfo;
                if (data.image) qrcodeEl.innerHTML = '<img src="data:image/png;base64,' + data.image + '" width="256" height="256" />';
                if (data.expires_at) startTimer(new Date(data.expires_at));
            } else if (data.state === 'paired') {
                statusEl.textContent = '✅ Successfully paired!';
                qrcodeEl.innerHTML = '<div class="icon">✅</div>';
                timerEl.textContent = 'You can close this page';
            } else if (data.state === 'timeout') {
                statusEl.textContent = '⏰ QR code expired' + attemptInfo;
                qrcodeEl.innerHTML = '<div class="icon">⏰</div>';
                timerEl.textContent = 'New QR code coming...';
            } else if (data.state === 'error') {
                statusEl.textContent = '❌ Error: ' + (data.error || 'Unknown');
                qrcodeEl.innerHTML = '<div class="icon">❌</div>';
                timerEl.textContent = 'Please restart the bridge';
            } else {
                statusEl.textContent = '⏳ Waiting for QR code...';
                qrcodeEl.innerHTML = '<div class="loading">⏳</div>';
                timerEl.textContent = 'QR code will appear shortly';
            }
        }
        
        function startTimer(expiresAt) {
            function update() {
                var remaining = Math.max(0, Math.floor((expiresAt - Date.now()) / 1000));
                timerEl.textContent = remaining > 0 ? 'Expires in ' + remaining + 's' : 'Generating new QR code...';
                if (remaining <= 0 && timerInterval) clearInterval(timerInterval);
            }
            update();
            timerInterval = setInterval(update, 1000);
        }
        
        function connectWebSocket() {
            var ws = new WebSocket((location.protocol === 'https:' ? 'wss:' : 'ws:') + '//' + location.host + '/qr/ws');
            ws.onmessage = function(e) { try { updateUI(JSON.parse(e.data)); } catch(err) {} };
            ws.onclose = function() { setTimeout(connectWebSocket, 3000); };
        }
        
        updateUI(initialState);
        connectWebSocket();
    </script>
</body>
</html>`
