package gorm

import (
	"testing"
)

func TestSanitizeFTSQuery(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "empty query",
			input:    "",
			expected: "",
		},
		{
			name:     "simple word",
			input:    "Augmentin",
			expected: "Augmentin",
		},
		{
			name:     "multiple words (implicit AND)",
			input:    "Augmentin 1g",
			expected: "Augmentin 1g", // 1g is alphanumeric, not quoted
		},
		{
			name:     "with OR operator",
			input:    "Augmentin OR Amoxicillin",
			expected: "Augmentin OR Amoxicillin",
		},
		{
			name:     "with AND operator",
			input:    "Augmentin AND 1g",
			expected: "Augmentin AND 1g", // 1g is alphanumeric, not quoted
		},
		{
			name:     "with NOT operator",
			input:    "Augmentin NOT expired",
			expected: "Augmentin NOT expired",
		},
		{
			name:     "with NEAR/n operator",
			input:    "Augmentin NEAR/5 antibiotic",
			expected: "Augmentin NEAR/5 antibiotic",
		},
		{
			name:     "prefix search",
			input:    "Aug*",
			expected: "\"Aug\"*",
		},
		{
			name:     "already quoted single word",
			input:    "\"Augmentin\"",
			expected: "\"Augmentin\"",
		},
		{
			name:     "special chars with dots",
			input:    "vitamin B12",
			expected: "vitamin B12",
		},
		{
			name:     "special chars with dash",
			input:    "Co-Amoxiclav",
			expected: "\"Co-Amoxiclav\"",
		},
		{
			name:     "column filter",
			input:    "medication:Augmentin",
			expected: "medication:Augmentin",
		},
		{
			name:     "column filter with special chars",
			input:    "medication:Co-Amox",
			expected: "medication:\"Co-Amox\"",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := SanitizeFTSQuery(tt.input)
			if result != tt.expected {
				t.Errorf("SanitizeFTSQuery(%q) = %q, want %q", tt.input, result, tt.expected)
			}
		})
	}
}

func TestNormalizeArabic(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "empty string",
			input:    "",
			expected: "",
		},
		{
			name:     "no arabic",
			input:    "Augmentin",
			expected: "Augmentin",
		},
		{
			name:     "alef with hamza above",
			input:    "أوجمنتين",
			expected: "اوجمنتين",
		},
		{
			name:     "alef with hamza below",
			input:    "إسلام",
			expected: "اسلام",
		},
		{
			name:     "alef with madda",
			input:    "آمين",
			expected: "امين", // آ -> ا (single alef, not double)
		},
		{
			name:     "teh marbuta",
			input:    "صيدلية",
			expected: "صيدليه",
		},
		{
			name:     "with diacritics",
			input:    "كَتَبَ",
			expected: "كتب",
		},
		{
			name:     "mixed arabic and english",
			input:    "أوجمنتين 1g",
			expected: "اوجمنتين 1g",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := NormalizeArabic(tt.input)
			if result != tt.expected {
				t.Errorf("NormalizeArabic(%q) = %q, want %q", tt.input, result, tt.expected)
			}
		})
	}
}

func TestBuildSearchQuery(t *testing.T) {
	tests := []struct {
		name            string
		query           string
		useOR           bool
		prefixSearch    bool
		normalizeArabic bool
		expected        string
	}{
		{
			name:            "empty query",
			query:           "",
			useOR:           false,
			prefixSearch:    false,
			normalizeArabic: false,
			expected:        "",
		},
		{
			name:            "simple with OR",
			query:           "Augmentin 1g",
			useOR:           true,
			prefixSearch:    false,
			normalizeArabic: false,
			expected:        "Augmentin OR 1g",
		},
		{
			name:            "with prefix search",
			query:           "Aug",
			useOR:           false,
			prefixSearch:    true,
			normalizeArabic: false,
			expected:        "Aug*",
		},
		{
			name:            "OR with prefix",
			query:           "Augmentin 1g",
			useOR:           true,
			prefixSearch:    true,
			normalizeArabic: false,
			expected:        "Augmentin OR 1g*",
		},
		{
			name:            "with Arabic normalization",
			query:           "أوجمنتين",
			useOR:           false,
			prefixSearch:    false,
			normalizeArabic: true,
			expected:        "اوجمنتين",
		},
		{
			name:            "preserves explicit operators - no auto OR added",
			query:           "Augmentin OR Amox",
			useOR:           true,
			prefixSearch:    true,
			normalizeArabic: false,
			expected:        "Augmentin OR Amox*", // Explicit OR preserved, no extra ORs
		},
		{
			name:            "explicit AND operator preserved",
			query:           "Augmentin AND 1g",
			useOR:           true,
			prefixSearch:    false,
			normalizeArabic: false,
			expected:        "Augmentin AND 1g", // AND preserved, auto-OR disabled
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := BuildSearchQuery(tt.query, tt.useOR, tt.prefixSearch, tt.normalizeArabic)
			if result != tt.expected {
				t.Errorf("BuildSearchQuery(%q, %v, %v, %v) = %q, want %q",
					tt.query, tt.useOR, tt.prefixSearch, tt.normalizeArabic, result, tt.expected)
			}
		})
	}
}

func TestBuildMedicationSearchQuery(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "empty",
			input:    "",
			expected: "",
		},
		{
			name:     "english medication",
			input:    "Augmentin 1g",
			expected: "Augmentin OR 1g*",
		},
		{
			name:     "arabic medication with hamza",
			input:    "أوجمنتين",
			expected: "اوجمنتين*",
		},
		{
			name:     "single word",
			input:    "Panadol",
			expected: "Panadol*",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := BuildMedicationSearchQuery(tt.input)
			if result != tt.expected {
				t.Errorf("BuildMedicationSearchQuery(%q) = %q, want %q", tt.input, result, tt.expected)
			}
		})
	}
}

func TestBuildProximityQuery(t *testing.T) {
	tests := []struct {
		name     string
		distance int
		terms    []string
		expected string
	}{
		{
			name:     "empty terms",
			distance: 5,
			terms:    []string{},
			expected: "",
		},
		{
			name:     "single term",
			distance: 5,
			terms:    []string{"Augmentin"},
			expected: "Augmentin",
		},
		{
			name:     "two terms",
			distance: 3,
			terms:    []string{"Augmentin", "1g"},
			expected: "\"Augmentin\" NEAR/3 \"1g\"",
		},
		{
			name:     "three terms",
			distance: 5,
			terms:    []string{"antibiotic", "Augmentin", "1g"},
			expected: "\"antibiotic\" NEAR/5 \"Augmentin\" NEAR/5 \"1g\"",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := BuildProximityQuery(tt.distance, tt.terms...)
			if result != tt.expected {
				t.Errorf("BuildProximityQuery(%d, %v) = %q, want %q",
					tt.distance, tt.terms, result, tt.expected)
			}
		})
	}
}

func TestHighlightMatches(t *testing.T) {
	tests := []struct {
		name     string
		text     string
		query    string
		startTag string
		endTag   string
		expected string
	}{
		{
			name:     "empty text",
			text:     "",
			query:    "test",
			startTag: "<b>",
			endTag:   "</b>",
			expected: "",
		},
		{
			name:     "empty query",
			text:     "test",
			query:    "",
			startTag: "<b>",
			endTag:   "</b>",
			expected: "test",
		},
		{
			name:     "simple match",
			text:     "Augmentin 1g tablets",
			query:    "Augmentin",
			startTag: "<mark>",
			endTag:   "</mark>",
			expected: "<mark>Augmentin</mark> 1g tablets",
		},
		{
			name:     "case insensitive",
			text:     "AUGMENTIN 1g tablets",
			query:    "augmentin",
			startTag: "<b>",
			endTag:   "</b>",
			expected: "<b>AUGMENTIN</b> 1g tablets",
		},
		{
			name:     "multiple terms",
			text:     "Augmentin 1g antibiotic",
			query:    "Augmentin antibiotic",
			startTag: "<b>",
			endTag:   "</b>",
			expected: "<b>Augmentin</b> 1g <b>antibiotic</b>",
		},
		{
			name:     "skip operators",
			text:     "Augmentin and Amoxicillin",
			query:    "Augmentin AND Amoxicillin",
			startTag: "<b>",
			endTag:   "</b>",
			expected: "<b>Augmentin</b> and <b>Amoxicillin</b>",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := HighlightMatches(tt.text, tt.query, tt.startTag, tt.endTag)
			if result != tt.expected {
				t.Errorf("HighlightMatches(%q, %q, %q, %q) = %q, want %q",
					tt.text, tt.query, tt.startTag, tt.endTag, result, tt.expected)
			}
		})
	}
}

func TestContainsSpecialChars(t *testing.T) {
	tests := []struct {
		input    string
		expected bool
	}{
		{"hello", false},
		{"hello123", false},
		{"hello-world", true},
		{"hello.world", true},
		{"hello/world", true},
		{"hello world", true},
		{"مرحبا", false}, // Arabic letters only
		{"1.5mg", true},
		{"", false},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result := containsSpecialChars(tt.input)
			if result != tt.expected {
				t.Errorf("containsSpecialChars(%q) = %v, want %v", tt.input, result, tt.expected)
			}
		})
	}
}
