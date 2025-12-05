package api

import (
	"embed"
	"io/fs"
	"net/http"

	"github.com/rs/zerolog"
)

//go:embed static/*
var staticFiles embed.FS

// NewRouter creates the HTTP router
func NewRouter(handlers *Handlers, log zerolog.Logger) http.Handler {
	mux := http.NewServeMux()

	// API routes
	mux.HandleFunc("GET /api/offers", handlers.GetOffers)
	mux.HandleFunc("GET /api/offers/{id}", handlers.GetOffer)
	mux.HandleFunc("GET /api/requests", handlers.GetRequests)
	mux.HandleFunc("GET /api/requests/{id}", handlers.GetRequest)
	mux.HandleFunc("GET /api/matches", handlers.GetMatches)
	mux.HandleFunc("POST /api/matches/{id}/confirm", handlers.ConfirmMatch)
	mux.HandleFunc("POST /api/matches/{id}/reject", handlers.RejectMatch)
	mux.HandleFunc("GET /api/stats", handlers.GetStats)
	mux.HandleFunc("GET /api/groups", handlers.GetGroups)
	mux.HandleFunc("POST /api/groups/sync", handlers.SyncGroups)
	mux.HandleFunc("PATCH /api/groups/{jid}", handlers.UpdateGroupMonitoring)

	// SSE endpoint
	mux.HandleFunc("GET /api/events", handlers.sseHub.ServeHTTP)

	// Static files (dashboard)
	staticFS, err := fs.Sub(staticFiles, "static")
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create static file system")
	}
	mux.Handle("GET /", http.FileServer(http.FS(staticFS)))

	// Middleware
	return corsMiddleware(loggingMiddleware(mux, log))
}

func loggingMiddleware(next http.Handler, log zerolog.Logger) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		log.Debug().
			Str("method", r.Method).
			Str("path", r.URL.Path).
			Str("remote", r.RemoteAddr).
			Msg("Request")
		next.ServeHTTP(w, r)
	})
}

func corsMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET, POST, PATCH, DELETE, OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type, Authorization")

		if r.Method == "OPTIONS" {
			w.WriteHeader(http.StatusNoContent)
			return
		}

		next.ServeHTTP(w, r)
	})
}
