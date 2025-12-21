// Package qr provides QR code handling for WhatsApp pairing.
// It supports terminal rendering and HTTP/WebSocket exposure for web dashboards.
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
	"time"

	"github.com/gorilla/websocket"
	"github.com/mdp/qrterminal/v3"
	"github.com/rs/zerolog"
	"github.com/skip2/go-qrcode"
)

// State represents the current QR/pairing state
type State string

const (
	StateWaiting  State = "waiting"  // Waiting for QR code
	StateReady    State = "ready"    // QR code available for scanning
	StateScanning State = "scanning" // User is scanning
	StatePaired   State = "paired"   // Successfully paired
	StateTimeout  State = "timeout"  // QR code expired
	StateError    State = "error"    // Error occurred
)

// QRUpdate represents a QR code update event
type QRUpdate struct {
	State     State     `json:"state"`
	Code      string    `json:"code,omitempty"`
	Image     string    `json:"image,omitempty"` // Base64 PNG image
	ExpiresAt time.Time `json:"expires_at,omitempty"`
	Error     string    `json:"error,omitempty"`
	Attempt   int       `json:"attempt,omitempty"`   // Current attempt number
	MaxRetry  int       `json:"max_retry,omitempty"` // Max retries (0 = infinite)
}

// Config holds QR handler configuration
type Config struct {
	RenderTerminal bool          // Render QR in terminal
	QRTimeout      time.Duration // QR code validity duration
	MaxRetries     int           // Max retry attempts (0 = infinite)
}

// DefaultConfig returns sensible defaults
func DefaultConfig() Config {
	return Config{
		RenderTerminal: true,
		QRTimeout:      60 * time.Second,
		MaxRetries:     5,
	}
}

// Handler manages QR code display and distribution
type Handler struct {
	logger zerolog.Logger
	mu     sync.RWMutex

	// Current state
	state      State
	qrCode     string
	qrImageB64 string // Base64 encoded PNG
	expiresAt  time.Time
	error      string
	attempt    int // Current attempt number
	maxRetry   int // Max retries

	// WebSocket clients
	clients   map[*websocket.Conn]bool
	clientsMu sync.RWMutex

	// Channel for QR updates
	updates chan QRUpdate
}

// New creates a new QR handler
func New(cfg Config, logger zerolog.Logger) *Handler {
	h := &Handler{
		logger:  logger.With().Str("component", "qr").Logger(),
		state:   StateWaiting,
		clients: make(map[*websocket.Conn]bool),
		updates: make(chan QRUpdate, 10),
	}

	// Start broadcast worker
	go h.broadcastWorker()

	return h
}

// HandleQRCode processes a new QR code from whatsmeow
func (h *Handler) HandleQRCode(code string, cfg Config) {
	// Generate base64 PNG image
	imageB64 := generateQRBase64(code)

	h.mu.Lock()
	h.qrCode = code
	h.qrImageB64 = imageB64
	h.state = StateReady
	h.expiresAt = time.Now().Add(cfg.QRTimeout)
	h.error = ""
	h.attempt++
	h.maxRetry = cfg.MaxRetries
	attempt := h.attempt
	maxRetry := h.maxRetry
	h.mu.Unlock()

	h.logger.Info().
		Str("state", string(StateReady)).
		Time("expires_at", h.expiresAt).
		Int("attempt", attempt).
		Int("max_retry", maxRetry).
		Bool("has_image", imageB64 != "").
		Msg("📱 QR code ready for scanning")

	// Render in terminal if enabled
	if cfg.RenderTerminal {
		h.renderTerminal(code, attempt, maxRetry)
	}

	// Broadcast to WebSocket clients
	h.updates <- QRUpdate{
		State:     StateReady,
		Code:      code,
		Image:     imageB64,
		ExpiresAt: h.expiresAt,
		Attempt:   attempt,
		MaxRetry:  maxRetry,
	}
}

// generateQRBase64 creates a base64-encoded PNG image of the QR code
func generateQRBase64(code string) string {
	qr, err := qrcode.New(code, qrcode.Medium)
	if err != nil {
		return ""
	}

	img := qr.Image(256)

	var buf bytes.Buffer
	if err := png.Encode(&buf, img); err != nil {
		return ""
	}

	return base64.StdEncoding.EncodeToString(buf.Bytes())
}

// HandleEvent processes QR channel events from whatsmeow
func (h *Handler) HandleEvent(event string, cfg Config) {
	h.mu.Lock()
	defer h.mu.Unlock()

	switch event {
	case "success":
		h.state = StatePaired
		h.qrCode = ""
		h.qrImageB64 = ""
		h.logger.Info().Msg("✅ WhatsApp paired successfully")
		h.updates <- QRUpdate{State: StatePaired}

	case "timeout":
		h.state = StateTimeout
		h.qrCode = ""
		h.qrImageB64 = ""
		h.logger.Warn().Msg("⏰ QR code expired")
		h.updates <- QRUpdate{State: StateTimeout}

	default:
		h.logger.Debug().Str("event", event).Msg("QR event received")
	}
}

// HandleError records an error state
func (h *Handler) HandleError(err error) {
	h.mu.Lock()
	h.state = StateError
	h.error = err.Error()
	h.qrCode = ""
	h.qrImageB64 = ""
	h.mu.Unlock()

	h.logger.Error().Err(err).Msg("QR pairing error")
	h.updates <- QRUpdate{State: StateError, Error: err.Error()}
}

// SetPaired marks the handler as paired (for already-paired devices)
func (h *Handler) SetPaired() {
	h.mu.Lock()
	h.state = StatePaired
	h.qrCode = ""
	h.qrImageB64 = ""
	h.mu.Unlock()
}

// renderTerminal renders the QR code in the terminal
func (h *Handler) renderTerminal(code string, attempt, maxRetry int) {
	h.logger.Info().Msg("Rendering QR code in terminal...")

	retryInfo := ""
	if maxRetry > 0 {
		retryInfo = fmt.Sprintf("  📊 Attempt %d of %d", attempt, maxRetry)
	} else {
		retryInfo = fmt.Sprintf("  📊 Attempt %d (unlimited retries)", attempt)
	}

	println("\n" + "═══════════════════════════════════════════════════════")
	println("  📱 Scan this QR code with WhatsApp to link device")
	println(retryInfo)
	println("═══════════════════════════════════════════════════════" + "\n")

	qrterminal.GenerateHalfBlock(code, qrterminal.L, os.Stdout)

	println("\n" + "═══════════════════════════════════════════════════════")
	println("  ⏰ QR code expires in 60 seconds")
	println("  🌐 Or visit http://localhost:5050/qr for web view")
	println("═══════════════════════════════════════════════════════" + "\n")
}

// GetState returns the current QR state
func (h *Handler) GetState() QRUpdate {
	h.mu.RLock()
	defer h.mu.RUnlock()

	return QRUpdate{
		State:     h.state,
		Code:      h.qrCode,
		Image:     h.qrImageB64,
		ExpiresAt: h.expiresAt,
		Error:     h.error,
		Attempt:   h.attempt,
		MaxRetry:  h.maxRetry,
	}
}

// IsPaired returns true if the device is paired
func (h *Handler) IsPaired() bool {
	h.mu.RLock()
	defer h.mu.RUnlock()
	return h.state == StatePaired
}

// --- HTTP Handlers ---

// HTTPHandler returns the QR code as JSON (for polling)
func (h *Handler) HTTPHandler(w http.ResponseWriter, r *http.Request) {
	state := h.GetState()

	w.Header().Set("Content-Type", "application/json")
	w.Header().Set("Cache-Control", "no-cache, no-store, must-revalidate")

	switch state.State {
	case StateReady:
		w.WriteHeader(http.StatusOK)
	case StatePaired:
		w.WriteHeader(http.StatusOK)
	case StateTimeout:
		w.WriteHeader(http.StatusGone)
	case StateError:
		w.WriteHeader(http.StatusInternalServerError)
	default:
		w.WriteHeader(http.StatusAccepted)
	}

	json.NewEncoder(w).Encode(state)
}

// HTMLHandler returns a simple HTML page with QR code
func (h *Handler) HTMLHandler(w http.ResponseWriter, r *http.Request) {
	state := h.GetState()

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Header().Set("Cache-Control", "no-cache, no-store, must-revalidate")

	html := `<!DOCTYPE html>
<html>
<head>
    <title>WhatsApp QR Code</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <style>
        body { 
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex; justify-content: center; align-items: center; 
            min-height: 100vh; margin: 0;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }
        .container {
            background: white; padding: 40px; border-radius: 20px;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3); text-align: center; max-width: 400px;
        }
        h1 { color: #333; margin-bottom: 10px; }
        p { color: #666; margin-bottom: 20px; }
        #qrcode { 
            margin: 20px auto; min-height: 260px; 
            display: flex; align-items: center; justify-content: center; 
        }
        #qrcode img { border-radius: 10px; box-shadow: 0 4px 12px rgba(0,0,0,0.1); }
        .status { 
            padding: 10px 20px; border-radius: 20px; 
            display: inline-block; margin-top: 20px; font-weight: 500;
        }
        .status.ready { background: #e3f2fd; color: #1976d2; }
        .status.paired { background: #e8f5e9; color: #388e3c; }
        .status.timeout { background: #fff3e0; color: #f57c00; }
        .status.error { background: #ffebee; color: #d32f2f; }
        .status.waiting { background: #f5f5f5; color: #757575; }
        .timer { font-size: 14px; color: #999; margin-top: 10px; }
        .icon { font-size: 80px; }
        .loading { font-size: 40px; animation: pulse 1.5s infinite; }
        @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.5; } }
        a { color: #1976d2; }
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
        var initialState = ` + toJSON(state) + `;
        var statusEl = document.getElementById('status');
        var timerEl = document.getElementById('timer');
        var qrcodeEl = document.getElementById('qrcode');
        var timerInterval = null;
        
        function updateUI(data) {
            statusEl.className = 'status ' + data.state;
            if (timerInterval) { clearInterval(timerInterval); timerInterval = null; }
            
            var attemptInfo = '';
            if (data.attempt > 0) {
                if (data.max_retry > 0) {
                    attemptInfo = ' (Attempt ' + data.attempt + '/' + data.max_retry + ')';
                } else {
                    attemptInfo = ' (Attempt ' + data.attempt + ')';
                }
            }
            
            if (data.state === 'ready') {
                statusEl.textContent = '⏳ Waiting for scan...' + attemptInfo;
                if (data.image) {
                    qrcodeEl.innerHTML = '<img src="data:image/png;base64,' + data.image + '" alt="QR Code" width="256" height="256" />';
                }
                if (data.expires_at) { startTimer(new Date(data.expires_at)); }
            } else if (data.state === 'paired') {
                statusEl.textContent = '✅ Successfully paired!';
                qrcodeEl.innerHTML = '<div class="icon">✅</div>';
                timerEl.textContent = 'You can close this page';
            } else if (data.state === 'timeout') {
                statusEl.textContent = '⏰ QR code expired' + attemptInfo;
                qrcodeEl.innerHTML = '<div class="icon">⏰</div>';
                timerEl.innerHTML = 'New QR code coming...';
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
                if (remaining > 0) {
                    timerEl.textContent = 'Expires in ' + remaining + 's';
                } else {
                    timerEl.innerHTML = 'Generating new QR code...';
                    if (timerInterval) clearInterval(timerInterval);
                }
            }
            update();
            timerInterval = setInterval(update, 1000);
        }
        
        function connectWebSocket() {
            var protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
            var ws = new WebSocket(protocol + '//' + location.host + '/qr/ws');
            ws.onmessage = function(e) {
                try { updateUI(JSON.parse(e.data)); } catch(err) { console.error(err); }
            };
            ws.onclose = function() { setTimeout(connectWebSocket, 3000); };
        }
        
        updateUI(initialState);
        connectWebSocket();
    </script>
</body>
</html>`

	w.Write([]byte(html))
}

func toJSON(v any) string {
	b, _ := json.Marshal(v)
	return string(b)
}

// --- WebSocket Handler ---

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool { return true },
}

// WebSocketHandler handles WebSocket connections for real-time QR updates
func (h *Handler) WebSocketHandler(w http.ResponseWriter, r *http.Request) {
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		h.logger.Error().Err(err).Msg("WebSocket upgrade failed")
		return
	}

	h.clientsMu.Lock()
	h.clients[conn] = true
	h.clientsMu.Unlock()

	h.logger.Debug().Str("remote", r.RemoteAddr).Msg("WebSocket client connected")

	// Send current state immediately
	state := h.GetState()
	data, _ := json.Marshal(state)
	conn.WriteMessage(websocket.TextMessage, data)

	defer func() {
		h.clientsMu.Lock()
		delete(h.clients, conn)
		h.clientsMu.Unlock()
		conn.Close()
	}()

	for {
		_, _, err := conn.ReadMessage()
		if err != nil {
			break
		}
	}
}

// broadcastWorker sends updates to all connected WebSocket clients
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

// Close shuts down the handler
func (h *Handler) Close() {
	close(h.updates)

	h.clientsMu.Lock()
	for conn := range h.clients {
		conn.Close()
	}
	h.clients = nil
	h.clientsMu.Unlock()
}
