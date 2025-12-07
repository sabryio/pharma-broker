package storage

import (
	"strings"
	"unicode"
)

// SanitizeFTSQuery sanitizes the user query for SQLite FTS5.
// It ensures that search terms are properly quoted to handle special characters
// like ".", "-", etc., while preserving FTS operators (AND, OR, NOT).
func SanitizeFTSQuery(query string) string {
	if query == "" {
		return ""
	}

	// Split by whitespace
	parts := strings.Fields(query)
	var processed []string

	for _, part := range parts {
		// If it's a keyword, keep it as is (FTS operators are case-sensitive)
		if part == "AND" || part == "OR" || part == "NOT" || part == "NEAR" {
			processed = append(processed, part)
			continue
		}

		// Check if it's already quoted
		if strings.HasPrefix(part, "\"") && strings.HasSuffix(part, "\"") {
			processed = append(processed, part)
			continue
		}

		// Check for valid prefix query (ending with *)
		if strings.HasSuffix(part, "*") {
			// Quote the part before the *
			term := strings.TrimSuffix(part, "*")
			// Escape existing quotes in term
			term = strings.ReplaceAll(term, "\"", "\"\"")
			processed = append(processed, "\""+term+"\"*")
			continue
		}

		// Escape existing quotes
		cleanPart := strings.ReplaceAll(part, "\"", "\"\"")

		// If the part contains non-alphanumeric chars (like . - /), wrap in quotes
		if containsSpecialChars(cleanPart) {
			processed = append(processed, "\""+cleanPart+"\"")
		} else {
			// Even single words are safer quoted in FTS5 if they might be keywords or reserved
			// but usually simple words are fine. However, consistent quoting is safer.
			// Let's quote everything that isn't an operator.
			processed = append(processed, "\""+cleanPart+"\"")
		}
	}

	return strings.Join(processed, " ")
}

func containsSpecialChars(s string) bool {
	for _, r := range s {
		if !unicode.IsLetter(r) && !unicode.IsNumber(r) {
			return true
		}
	}
	return false
}
