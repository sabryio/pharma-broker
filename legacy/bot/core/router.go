package core

import (
	"context"
	"sync"

	"github.com/rs/zerolog"
)

// CommandRouter implements Router with middleware support.
type CommandRouter struct {
	handlers   map[string]CommandHandler
	middleware []Middleware
	mu         sync.RWMutex
	log        zerolog.Logger
}

// NewRouter creates a new command router.
func NewRouter(log zerolog.Logger) *CommandRouter {
	return &CommandRouter{
		handlers: make(map[string]CommandHandler),
		log:      log.With().Str("component", "bot-router").Logger(),
	}
}

// Use adds middleware to all commands.
func (r *CommandRouter) Use(mw Middleware) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.middleware = append(r.middleware, mw)
}

// Register adds a command handler.
func (r *CommandRouter) Register(handler CommandHandler) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.handlers[handler.Name()] = handler
	r.log.Debug().Str("command", handler.Name()).Msg("Registered command handler")
}

// Handle routes a command to the appropriate handler.
func (r *CommandRouter) Handle(ctx context.Context, cmd *Command, msg *Message) *Response {
	r.mu.RLock()
	handler, exists := r.handlers[cmd.Name]
	middleware := r.middleware
	r.mu.RUnlock()

	if !exists {
		return &Response{
			Text:      "❌ Unknown command. Type /help for available commands.",
			ParseMode: ParseModeText,
		}
	}

	// Apply middleware in reverse order (last added wraps outermost)
	wrapped := handler
	for i := len(middleware) - 1; i >= 0; i-- {
		wrapped = middleware[i](wrapped)
	}

	resp := wrapped.Handle(ctx, cmd, msg)
	return &resp
}

// GetHandler returns a handler by name.
func (r *CommandRouter) GetHandler(name string) (CommandHandler, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	h, ok := r.handlers[name]
	return h, ok
}

// Commands returns all registered command names.
func (r *CommandRouter) Commands() []string {
	r.mu.RLock()
	defer r.mu.RUnlock()
	names := make([]string, 0, len(r.handlers))
	for name := range r.handlers {
		names = append(names, name)
	}
	return names
}
