// Package qradapter provides an adapter for the QR handler.
package qradapter

import (
	"github.com/gin-gonic/gin"
	"github.com/rs/zerolog"

	"pharma-bridge/ports"
	"pharma-bridge/qr"
)

// HandlerAdapter wraps qr.Handler to implement ports.QRHandler.
type HandlerAdapter struct {
	handler *qr.Handler
	config  qr.Config
}

// NewHandlerAdapter creates a new QR handler adapter.
func NewHandlerAdapter(cfg qr.Config, logger zerolog.Logger) *HandlerAdapter {
	return &HandlerAdapter{
		handler: qr.New(cfg, logger),
		config:  cfg,
	}
}

// HandleQRCode processes a new QR code.
func (a *HandlerAdapter) HandleQRCode(code string) {
	a.handler.HandleQRCode(code, a.config)
}

// HandleEvent processes QR events.
func (a *HandlerAdapter) HandleEvent(event string) {
	a.handler.HandleEvent(event, a.config)
}

// HandleError records an error state.
func (a *HandlerAdapter) HandleError(err error) {
	a.handler.HandleError(err)
}

// SetPaired marks the handler as paired.
func (a *HandlerAdapter) SetPaired() {
	a.handler.SetPaired()
}

// IsPaired returns true if paired.
func (a *HandlerAdapter) IsPaired() bool {
	return a.handler.IsPaired()
}

// Close shuts down the handler.
func (a *HandlerAdapter) Close() {
	a.handler.Close()
}

// RegisterRoutes registers QR routes on a Gin router group.
func (a *HandlerAdapter) RegisterRoutes(rg *gin.RouterGroup) {
	a.handler.RegisterRoutes(rg)
}

var _ ports.QRHandler = (*HandlerAdapter)(nil)
