package ai

import (
	"testing"
)

func TestLevenshteinDistance(t *testing.T) {
	tests := []struct {
		name     string
		s1       string
		s2       string
		expected int
	}{
		// Basic cases
		{"identical", "hello", "hello", 0},
		{"empty both", "", "", 0},
		{"empty first", "", "abc", 3},
		{"empty second", "abc", "", 3},

		// Single operations
		{"one insertion", "cat", "cats", 1},
		{"one deletion", "cats", "cat", 1},
		{"one substitution", "cat", "hat", 1},

		// Multiple operations
		{"two substitutions", "cat", "dog", 3},
		{"kitten to sitting", "kitten", "sitting", 3},

		// Arabic text
		{"arabic missing char", "ريبلسس", "ريبلسوس", 1},
		{"arabic different vowel", "ديكابيبتايل", "ديكابيبتيل", 1},
		{"arabic extra char", "زولادكسس", "زولادكس", 1},
		// Note: actual distance varies, just verify it runs without panic
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			result := LevenshteinDistance(tc.s1, tc.s2)
			if result != tc.expected {
				t.Errorf("LevenshteinDistance(%q, %q) = %d, want %d", tc.s1, tc.s2, result, tc.expected)
			}
		})
	}
}

func TestFuzzyFindBest(t *testing.T) {
	mappings := map[string]string{
		"ريبلسوس":    "Rybelsus",
		"ديكابيبتيل": "Decapeptyl",
		"اوفتريل":    "Ovitrelle",
		"ميريوفيرت":  "Mireofert",
		"زولادكس":    "Zoladex",
	}

	tests := []struct {
		name            string
		query           string
		maxDistance     int
		expectedEnglish string
		expectedConf    MatchConfidence
		shouldMatch     bool
	}{
		// Exact matches (distance 0)
		{"exact match", "ريبلسوس", 2, "Rybelsus", ConfidenceExact, true},
		{"exact normalized", "اوفتريل", 2, "Ovitrelle", ConfidenceExact, true},

		// Fuzzy matches (distance 1-2)
		{"missing char", "ريبلسس", 2, "Rybelsus", ConfidenceFuzzy, true},
		{"different vowel", "ديكابيبتايل", 2, "Decapeptyl", ConfidenceFuzzy, true},
		{"extra char", "زولادكسس", 2, "Zoladex", ConfidenceFuzzy, true},

		// No match (distance > maxDistance)
		{"too different", "غير موجود", 2, "", "", false},
		{"completely different", "سكسندا", 2, "", "", false},

		// Edge cases
		{"empty query", "", 2, "", "", false},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			result := FuzzyFindBest(tc.query, mappings, tc.maxDistance)

			if tc.shouldMatch {
				if result == nil {
					t.Errorf("FuzzyFindBest(%q) = nil, want match for %q", tc.query, tc.expectedEnglish)
					return
				}
				if result.EnglishName != tc.expectedEnglish {
					t.Errorf("FuzzyFindBest(%q).EnglishName = %q, want %q", tc.query, result.EnglishName, tc.expectedEnglish)
				}
				if result.Confidence != tc.expectedConf {
					t.Errorf("FuzzyFindBest(%q).Confidence = %q, want %q", tc.query, result.Confidence, tc.expectedConf)
				}
			} else {
				if result != nil {
					t.Errorf("FuzzyFindBest(%q) = %v, want nil", tc.query, result)
				}
			}
		})
	}
}

func TestFuzzyContains(t *testing.T) {
	tests := []struct {
		name      string
		haystack  string
		needle    string
		maxErrors int
		expected  bool
	}{
		{"exact substring", "hello world", "world", 0, true},
		{"no match", "hello there", "world", 0, false},
		{"empty needle", "hello", "", 0, true},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			result := fuzzyContains(tc.haystack, tc.needle, tc.maxErrors)
			if result != tc.expected {
				t.Errorf("fuzzyContains(%q, %q, %d) = %v, want %v",
					tc.haystack, tc.needle, tc.maxErrors, result, tc.expected)
			}
		})
	}
}

func TestMatchConfidenceConstants(t *testing.T) {
	// Ensure confidence constants are defined correctly
	if ConfidenceExact != "EXACT" {
		t.Errorf("ConfidenceExact = %q, want EXACT", ConfidenceExact)
	}
	if ConfidenceFuzzy != "FUZZY" {
		t.Errorf("ConfidenceFuzzy = %q, want FUZZY", ConfidenceFuzzy)
	}
	if ConfidenceVector != "VECTOR" {
		t.Errorf("ConfidenceVector = %q, want VECTOR", ConfidenceVector)
	}
	if ConfidenceTransliterated != "TRANSLITERATED" {
		t.Errorf("ConfidenceTransliterated = %q, want TRANSLITERATED", ConfidenceTransliterated)
	}
}
