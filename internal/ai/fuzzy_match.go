package ai

// LevenshteinDistance calculates the minimum number of single-character edits
// (insertions, deletions, substitutions) required to change one string into another.
// This is useful for fuzzy matching medication names with typos.
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

// FuzzyMatchResult represents a fuzzy match result
type FuzzyMatchResult struct {
	ArabicKey   string
	EnglishName string
	Distance    int
	Confidence  MatchConfidence
}

// MatchConfidence indicates how the match was made
type MatchConfidence string

const (
	ConfidenceExact          MatchConfidence = "EXACT"
	ConfidenceFuzzy          MatchConfidence = "FUZZY"
	ConfidenceVector         MatchConfidence = "VECTOR"
	ConfidenceTransliterated MatchConfidence = "TRANSLITERATED"
)

// FuzzyFindBest finds the best fuzzy match for a query from a set of mappings.
// Returns nil if no match within maxDistance is found.
func FuzzyFindBest(query string, mappings map[string]string, maxDistance int) *FuzzyMatchResult {
	queryNormalized := NormalizeForMatching(query)

	var bestMatch *FuzzyMatchResult
	bestDistance := maxDistance + 1

	for arabic, english := range mappings {
		arabicNormalized := NormalizeForMatching(arabic)

		// First check for exact match (distance 0)
		if queryNormalized == arabicNormalized {
			return &FuzzyMatchResult{
				ArabicKey:   arabic,
				EnglishName: english,
				Distance:    0,
				Confidence:  ConfidenceExact,
			}
		}

		// Calculate Levenshtein distance
		distance := LevenshteinDistance(queryNormalized, arabicNormalized)

		if distance <= maxDistance && distance < bestDistance {
			bestDistance = distance
			bestMatch = &FuzzyMatchResult{
				ArabicKey:   arabic,
				EnglishName: english,
				Distance:    distance,
				Confidence:  ConfidenceFuzzy,
			}
		}
	}

	return bestMatch
}

// FuzzyFilterMappings returns mappings that fuzzy-match content within maxDistance.
// This is slower than keyword matching but catches typos.
func FuzzyFilterMappings(content string, mappings map[string]string, maxDistance int) map[string]string {
	result := make(map[string]string)
	contentNormalized := NormalizeForMatching(content)

	for arabic, english := range mappings {
		arabicNormalized := NormalizeForMatching(arabic)

		// Check if arabic key appears (with fuzzy tolerance) in content
		// We use a sliding window approach for substring fuzzy matching
		if fuzzyContains(contentNormalized, arabicNormalized, maxDistance) {
			result[arabic] = english
		}
	}

	return result
}

// fuzzyContains checks if needle appears in haystack with at most maxErrors
func fuzzyContains(haystack, needle string, maxErrors int) bool {
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
