package main

import (
	"fmt"

	"pharmabroker/internal/ai"
)

// Playground for testing fuzzy matching with Levenshtein distance

func main() {
	fmt.Println("=== Playground: Fuzzy Matching Test ===")
	fmt.Println()

	// Test Levenshtein distance
	distanceTests := []struct {
		s1       string
		s2       string
		expected int
	}{
		{"ريبلسس", "ريبلسوس", 1},         // Missing و (1 char)
		{"ديكابيبتايل", "ديكابيبتيل", 1}, // Different vowel (1 char)
		{"cat", "hat", 1},
		{"kitten", "sitting", 3},
		{"", "abc", 3},
		{"abc", "", 3},
		{"same", "same", 0},
	}

	fmt.Println("Levenshtein Distance Tests:")
	for _, tc := range distanceTests {
		distance := ai.LevenshteinDistance(tc.s1, tc.s2)
		status := "✓"
		if distance != tc.expected {
			status = fmt.Sprintf("✗ (got %d)", distance)
		}
		fmt.Printf("  %q vs %q = %d %s\n", tc.s1, tc.s2, distance, status)
	}

	fmt.Println()

	// Test fuzzy matching
	mappings := map[string]string{
		"ريبلسوس":    "Rybelsus",
		"ديكابيبتيل": "Decapeptyl",
		"اوفتريل":    "Ovitrelle",
		"ميريوفيرت":  "Mireofert",
		"زولادكس":    "Zoladex",
	}

	fuzzyTests := []struct {
		query    string
		expected string
	}{
		{"ريبلسس", "Rybelsus"},        // Missing و, distance 1
		{"ديكابيبتايل", "Decapeptyl"}, // Different vowel, distance 1
		{"زولادكسس", "Zoladex"},       // Extra س, distance 1
		{"غير موجود", ""},             // No match within distance 2
	}

	fmt.Println("Fuzzy Match Tests (max distance = 2):")
	for _, tc := range fuzzyTests {
		result := ai.FuzzyFindBest(tc.query, mappings, 2)
		if result == nil {
			if tc.expected == "" {
				fmt.Printf("  %s → (no match) ✓\n", tc.query)
			} else {
				fmt.Printf("  %s → (no match) ✗ expected %s\n", tc.query, tc.expected)
			}
		} else {
			status := "✓"
			if result.EnglishName != tc.expected {
				status = fmt.Sprintf("✗ expected %s", tc.expected)
			}
			fmt.Printf("  %s → %s (distance=%d, confidence=%s) %s\n",
				tc.query, result.EnglishName, result.Distance, result.Confidence, status)
		}
	}

	fmt.Println()
	fmt.Println("Phase 2: Fuzzy Matching implementation complete!")
}
