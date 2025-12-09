package parsing

import (
	"strings"
)

// fuzzyMatch returns a similarity score between 0 and 1 using Levenshtein distance.
// For very long strings, it falls back to a simpler substring match for performance.
func fuzzyMatch(s1, s2 string) float64 {
	// Normalize strings: lowercase and trim
	s1 = strings.ToLower(strings.TrimSpace(s1))
	s2 = strings.ToLower(strings.TrimSpace(s2))

	if s1 == s2 {
		return 1.0
	}

	if len(s1) == 0 || len(s2) == 0 {
		return 0.0
	}

	// Optimize: For very long strings, use simpler comparison
	if len(s1) > MaxLevenshteinLength || len(s2) > MaxLevenshteinLength {
		return simpleSubstringMatch(s1, s2)
	}

	// Calculate Levenshtein distance
	dist := levenshteinDistance(s1, s2)
	maxLen := max(len(s1), len(s2))

	// Convert to similarity score (1 - normalized distance)
	similarity := 1.0 - float64(dist)/float64(maxLen)
	return similarity
}

// simpleSubstringMatch provides a fast fallback for long strings
func simpleSubstringMatch(s1, s2 string) float64 {
	if strings.Contains(s1, s2) || strings.Contains(s2, s1) {
		shorter := len(s1)
		if len(s2) < shorter {
			shorter = len(s2)
		}
		longer := max(len(s1), len(s2))
		return float64(shorter) / float64(longer)
	}
	return 0.0
}

// levenshteinDistance calculates the edit distance between two strings
func levenshteinDistance(s1, s2 string) int {
	if len(s1) == 0 {
		return len(s2)
	}
	if len(s2) == 0 {
		return len(s1)
	}

	// Use two rows instead of full matrix for memory efficiency
	prev := make([]int, len(s2)+1)
	curr := make([]int, len(s2)+1)

	// Initialize first row
	for j := range prev {
		prev[j] = j
	}

	for i := 1; i <= len(s1); i++ {
		curr[0] = i
		for j := 1; j <= len(s2); j++ {
			cost := 1
			if s1[i-1] == s2[j-1] {
				cost = 0
			}
			curr[j] = min(
				prev[j]+1,      // deletion
				curr[j-1]+1,    // insertion
				prev[j-1]+cost, // substitution
			)
		}
		prev, curr = curr, prev
	}

	return prev[len(s2)]
}

// generateTrigrams splits a string into 3-character substrings
func generateTrigrams(s string) []string {
	runes := []rune(s)
	if len(runes) < 3 {
		return nil
	}
	var trigrams []string
	for i := 0; i < len(runes)-2; i++ {
		trigrams = append(trigrams, string(runes[i:i+3]))
	}
	return trigrams
}

// sanitizeForFTS prepares a string for FTS5 search
func sanitizeForFTS(s string) string {
	// Remove common punctuation and special chars
	s = strings.ReplaceAll(s, "\"", "")
	s = strings.ReplaceAll(s, "'", "")
	s = strings.ReplaceAll(s, "*", " ") // Remove existing wildcards
	s = strings.ReplaceAll(s, "(", " ")
	s = strings.ReplaceAll(s, ")", " ")
	s = strings.ReplaceAll(s, "-", " ")

	tokens := strings.Fields(s)
	if len(tokens) == 0 {
		return ""
	}

	// Join with OR to broaden search (finding candidates)
	// We rely on subsequent scoring to filter bad matches
	return strings.Join(tokens, " OR ")
}

func min(a, b, c int) int {
	if a < b {
		if a < c {
			return a
		}
		return c
	}
	if b < c {
		return b
	}
	return c
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}
