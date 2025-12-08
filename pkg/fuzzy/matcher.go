// Package fuzzy provides fuzzy string matching utilities.
// This package has no external dependencies.
package fuzzy

import "pharmabroker/pkg/arabic"

// MatchConfidence indicates how the match was made
type MatchConfidence string

const (
	ConfidenceExact          MatchConfidence = "EXACT"
	ConfidenceFuzzy          MatchConfidence = "FUZZY"
	ConfidenceVector         MatchConfidence = "VECTOR"
	ConfidenceTransliterated MatchConfidence = "TRANSLITERATED"
)

// MatchResult represents a fuzzy match result
type MatchResult struct {
	Key        string          // Original key (e.g., Arabic name)
	Value      string          // Mapped value (e.g., English name)
	Distance   int             // Levenshtein distance
	Confidence MatchConfidence // How the match was made
}

// LevenshteinDistance calculates the minimum number of single-character edits
// (insertions, deletions, substitutions) required to change one string into another.
func LevenshteinDistance(s1, s2 string) int {
	r1 := []rune(s1)
	r2 := []rune(s2)
	len1 := len(r1)
	len2 := len(r2)

	if len1 == 0 {
		return len2
	}
	if len2 == 0 {
		return len1
	}

	// Create distance matrix
	matrix := make([][]int, len1+1)
	for i := range matrix {
		matrix[i] = make([]int, len2+1)
		matrix[i][0] = i
	}
	for j := 0; j <= len2; j++ {
		matrix[0][j] = j
	}

	// Fill in the matrix
	for i := 1; i <= len1; i++ {
		for j := 1; j <= len2; j++ {
			cost := 0
			if r1[i-1] != r2[j-1] {
				cost = 1
			}
			matrix[i][j] = min(
				matrix[i-1][j]+1,      // deletion
				matrix[i][j-1]+1,      // insertion
				matrix[i-1][j-1]+cost, // substitution
			)
		}
	}

	return matrix[len1][len2]
}

// FindBest finds the best fuzzy match for a query from a set of mappings.
// Returns nil if no match within maxDistance is found.
func FindBest(query string, mappings map[string]string, maxDistance int) *MatchResult {
	queryNormalized := arabic.NormalizeForMatching(query)

	var bestMatch *MatchResult
	bestDistance := maxDistance + 1

	for key, value := range mappings {
		keyNormalized := arabic.NormalizeForMatching(key)

		// First check for exact match (distance 0)
		if queryNormalized == keyNormalized {
			return &MatchResult{
				Key:        key,
				Value:      value,
				Distance:   0,
				Confidence: ConfidenceExact,
			}
		}

		// Calculate Levenshtein distance
		distance := LevenshteinDistance(queryNormalized, keyNormalized)

		if distance <= maxDistance && distance < bestDistance {
			bestDistance = distance
			bestMatch = &MatchResult{
				Key:        key,
				Value:      value,
				Distance:   distance,
				Confidence: ConfidenceFuzzy,
			}
		}
	}

	return bestMatch
}

// FilterMappings returns mappings that fuzzy-match content within maxDistance.
func FilterMappings(content string, mappings map[string]string, maxDistance int) map[string]string {
	result := make(map[string]string)
	contentNormalized := arabic.NormalizeForMatching(content)

	for key, value := range mappings {
		keyNormalized := arabic.NormalizeForMatching(key)

		// Check if key appears (with fuzzy tolerance) in content
		if Contains(contentNormalized, keyNormalized, maxDistance) {
			result[key] = value
		}
	}

	return result
}

// Contains checks if needle appears in haystack with at most maxErrors
func Contains(haystack, needle string, maxErrors int) bool {
	if len(needle) == 0 {
		return true
	}

	needleRunes := []rune(needle)
	haystackRunes := []rune(haystack)
	needleLen := len(needleRunes)
	haystackLen := len(haystackRunes)

	if needleLen > haystackLen {
		return LevenshteinDistance(haystack, needle) <= maxErrors
	}

	// Sliding window
	for i := 0; i <= haystackLen-needleLen+maxErrors; i++ {
		end := i + needleLen + maxErrors
		if end > haystackLen {
			end = haystackLen
		}
		window := string(haystackRunes[i:end])
		if LevenshteinDistance(window, needle) <= maxErrors {
			return true
		}
	}

	return false
}
