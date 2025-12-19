package handlers

import (
	"net/http"
	"strconv"

	"github.com/gin-gonic/gin"
)

// Gin response helpers for consistent API responses

// SuccessGin sends a success response
func SuccessGin(c *gin.Context, data any) {
	c.JSON(http.StatusOK, Response{Success: true, Data: data})
}

// SuccessWithMetaGin sends a success response with pagination metadata
func SuccessWithMetaGin(c *gin.Context, data any, meta *Meta) {
	c.JSON(http.StatusOK, Response{Success: true, Data: data, Meta: meta})
}

// ErrorGin sends an error response with the specified status code
func ErrorGin(c *gin.Context, status int, err *APIError) {
	c.JSON(status, Response{Success: false, Error: err})
}

// BadRequestGin sends a 400 error response
func BadRequestGin(c *gin.Context, message string) {
	ErrorGin(c, http.StatusBadRequest, ErrBadRequest(message))
}

// NotFoundGin sends a 404 error response
func NotFoundGin(c *gin.Context, err *APIError) {
	ErrorGin(c, http.StatusNotFound, err)
}

// InternalErrorGin sends a 500 error response
func InternalErrorGin(c *gin.Context, message string) {
	ErrorGin(c, http.StatusInternalServerError, ErrInternal(message))
}

// DatabaseErrorGin sends a database error response
func DatabaseErrorGin(c *gin.Context, message string) {
	ErrorGin(c, http.StatusInternalServerError, ErrDatabase(message))
}

// Pagination helpers

// GetPaginationGin extracts pagination parameters from Gin context
func GetPaginationGin(c *gin.Context) (limit, offset int) {
	limit = 20
	offset = 0

	if l := c.DefaultQuery("limit", "20"); l != "" {
		if parsed, err := strconv.Atoi(l); err == nil && parsed > 0 && parsed <= 100 {
			limit = parsed
		}
	}

	if o := c.DefaultQuery("offset", "0"); o != "" {
		if parsed, err := strconv.Atoi(o); err == nil && parsed >= 0 {
			offset = parsed
		}
	}

	return limit, offset
}

// GetPathIDGin extracts a path parameter and validates it's not empty
// Returns the ID and true if valid, or sends an error response and returns false
func GetPathIDGin(c *gin.Context, param string) (string, bool) {
	id := c.Param(param)
	if id == "" {
		BadRequestGin(c, "Missing "+param)
		return "", false
	}
	return id, true
}

// GetQueryString extracts an optional query string parameter
func GetQueryString(c *gin.Context, key string) string {
	return c.Query(key)
}

// GetQueryInt extracts an optional query int parameter with a default
func GetQueryInt(c *gin.Context, key string, defaultVal int) int {
	if val := c.Query(key); val != "" {
		if parsed, err := strconv.Atoi(val); err == nil {
			return parsed
		}
	}
	return defaultVal
}

// GetQueryBool extracts an optional query bool parameter with a default
func GetQueryBool(c *gin.Context, key string, defaultVal bool) bool {
	if val := c.Query(key); val != "" {
		switch val {
		case "true", "1", "yes":
			return true
		case "false", "0", "no":
			return false
		}
	}
	return defaultVal
}

// BindJSONGin binds request body to a struct with validation
// Returns true if binding succeeds, or sends error response and returns false
func BindJSONGin[T any](c *gin.Context, req *T) bool {
	if err := c.ShouldBindJSON(req); err != nil {
		ErrorGin(c, http.StatusBadRequest, ErrValidation(err.Error()))
		return false
	}
	return true
}

// GetTraceID extracts the trace ID from Gin context (set by middleware)
func GetTraceIDFromGin(c *gin.Context) string {
	if id, exists := c.Get("trace_id"); exists {
		if traceID, ok := id.(string); ok {
			return traceID
		}
	}
	return ""
}
