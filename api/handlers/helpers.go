package handlers

import (
	"encoding/json"
	"net/http"
	"strconv"
)

// writeJSON writes a JSON response
func writeJSON(w http.ResponseWriter, status int, data interface{}) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	json.NewEncoder(w).Encode(data)
}

// success writes a success response
func success(w http.ResponseWriter, data interface{}) {
	writeJSON(w, http.StatusOK, Response{Success: true, Data: data})
}

// successWithMeta writes a success response with metadata
func successWithMeta(w http.ResponseWriter, data interface{}, m *Meta) {
	writeJSON(w, http.StatusOK, Response{Success: true, Data: data, Meta: m})
}

// error writes an error response (legacy adapter)
func apiError(w http.ResponseWriter, status int, msg string) {
	errorWithCode(w, status, ErrBadRequest(msg))
}

// errorWithCode writes a structured error response
func errorWithCode(w http.ResponseWriter, status int, apiErr *APIError) {
	writeJSON(w, status, Response{Success: false, Error: apiErr})
}

// getPagination parses pagination parameters from request
func getPagination(r *http.Request) (int, int) {
	limit := 20
	offset := 0

	if l := r.URL.Query().Get("limit"); l != "" {
		if parsed, err := strconv.Atoi(l); err == nil && parsed > 0 && parsed <= 100 {
			limit = parsed
		}
	}

	if o := r.URL.Query().Get("offset"); o != "" {
		if parsed, err := strconv.Atoi(o); err == nil && parsed >= 0 {
			offset = parsed
		}
	}

	return limit, offset
}
