package handlers

import (
	"encoding/json"
	"net/http"
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
