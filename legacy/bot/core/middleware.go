package core

import (
	"context"

	"github.com/rs/zerolog"
)

// PhoneAuthorizer authorizes users by phone number.
type PhoneAuthorizer struct {
	authorized map[string]bool
	log        zerolog.Logger
}

// NewPhoneAuthorizer creates an authorizer with a list of authorized phones.
func NewPhoneAuthorizer(phones []string, log zerolog.Logger) *PhoneAuthorizer {
	authorized := make(map[string]bool)
	for _, p := range phones {
		normalized := NormalizePhone(p)
		authorized[normalized] = true
	}
	return &PhoneAuthorizer{
		authorized: authorized,
		log:        log.With().Str("component", "bot-auth").Logger(),
	}
}

// IsAuthorized checks if a phone number is authorized.
func (a *PhoneAuthorizer) IsAuthorized(_ context.Context, senderID string) bool {
	// Extract phone from platform-specific ID
	phone := extractPhone(senderID)
	return a.authorized[phone]
}

// AddPhone adds a phone number to the authorized list.
func (a *PhoneAuthorizer) AddPhone(phone string) {
	normalized := NormalizePhone(phone)
	a.authorized[normalized] = true
}

// RemovePhone removes a phone number from the authorized list.
func (a *PhoneAuthorizer) RemovePhone(phone string) {
	normalized := NormalizePhone(phone)
	delete(a.authorized, normalized)
}

// extractPhone extracts phone from various ID formats.
func extractPhone(id string) string {
	// Handle WhatsApp JID format: 201234567890@s.whatsapp.net
	for i := 0; i < len(id); i++ {
		if id[i] == '@' {
			return id[:i]
		}
	}
	return id
}

// AuthMiddleware creates authorization middleware.
func AuthMiddleware(auth Authorizer, log zerolog.Logger) Middleware {
	return func(next CommandHandler) CommandHandler {
		return &authHandler{
			next: next,
			auth: auth,
			log:  log,
		}
	}
}

type authHandler struct {
	next CommandHandler
	auth Authorizer
	log  zerolog.Logger
}

func (h *authHandler) Name() string        { return h.next.Name() }
func (h *authHandler) Description() string { return h.next.Description() }
func (h *authHandler) Usage() string       { return h.next.Usage() }

func (h *authHandler) Handle(ctx context.Context, cmd *Command, msg *Message) Response {
	if !h.auth.IsAuthorized(ctx, msg.SenderID) {
		h.log.Warn().
			Str("sender", msg.SenderID).
			Str("command", cmd.Name).
			Msg("Unauthorized command attempt")
		return Response{} // Silent ignore for unauthorized users
	}
	return h.next.Handle(ctx, cmd, msg)
}

// LoggingMiddleware logs command execution.
func LoggingMiddleware(log zerolog.Logger) Middleware {
	return func(next CommandHandler) CommandHandler {
		return &loggingHandler{next: next, log: log}
	}
}

type loggingHandler struct {
	next CommandHandler
	log  zerolog.Logger
}

func (h *loggingHandler) Name() string        { return h.next.Name() }
func (h *loggingHandler) Description() string { return h.next.Description() }
func (h *loggingHandler) Usage() string       { return h.next.Usage() }

func (h *loggingHandler) Handle(ctx context.Context, cmd *Command, msg *Message) Response {
	h.log.Info().
		Str("command", cmd.Name).
		Strs("args", cmd.Args).
		Str("sender", msg.SenderID).
		Str("platform", string(msg.Platform)).
		Msg("Executing bot command")

	resp := h.next.Handle(ctx, cmd, msg)

	h.log.Debug().
		Str("command", cmd.Name).
		Bool("has_response", resp.Text != "").
		Msg("Command completed")

	return resp
}
