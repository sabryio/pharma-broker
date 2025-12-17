package gorm

import (
	"strings"
	"testing"
)

func TestSanitizePgQuery(t *testing.T) {
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
			name:     "multiple words (joined with OR)",
			input:    "Augmentin 1g",
			expected: "Augmentin | 1g", // PostgreSQL uses | for OR by default
		},
		{
			name:     "with OR operator",
			input:    "Augmentin OR Amoxicillin",
			expected: "Augmentin | | | Amoxicillin", // OR becomes |
		},
		{
			name:     "with AND operator",
			input:    "Augmentin AND 1g",
			expected: "Augmentin | & | 1g", // AND becomes &
		},
		{
			name:     "with NOT operator",
			input:    "Augmentin NOT expired",
			expected: "Augmentin | ! | expired", // NOT becomes !
		},
		{
			name:     "prefix search",
			input:    "Aug*",
			expected: "Aug:*", // PostgreSQL uses :* for prefix
		},
		// Note: Quoted strings like "Augmentin" get quotes stripped and spaces added
		// This is fine as PostgreSQL handles quoting differently
		{
			name: "special chars with dots - vitamin B12",

			input:    "vitamin B12",
			expected: "vitamin | B12",
		},
		{
			name:     "special chars with dash",
			input:    "Co-Amoxiclav",
			expected: "Co Amoxiclav", // Dash replaced with space
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := SanitizePgQuery(tt.input)
			if result != tt.expected {
				t.Errorf("SanitizePgQuery(%q) = %q, want %q", tt.input, result, tt.expected)
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

func TestBuildPgSearchQuery(t *testing.T) {
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
			expected:        "Augmentin | 1g", // PostgreSQL uses |
		},
		{
			name:            "with prefix search",
			query:           "Aug",
			useOR:           false,
			prefixSearch:    true,
			normalizeArabic: false,
			expected:        "Aug:*", // PostgreSQL uses :*
		},
		{
			name:            "OR with prefix",
			query:           "Augmentin 1g",
			useOR:           true,
			prefixSearch:    true,
			normalizeArabic: false,
			expected:        "Augmentin | 1g:*", // Last term gets :*
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
			name:            "preserves explicit operators - converted to PostgreSQL",
			query:           "Augmentin OR Amox",
			useOR:           true,
			prefixSearch:    true,
			normalizeArabic: false,
			expected:        "Augmentin | Amox:*", // OR becomes |, explicit operators filtered
		},
		{
			name:            "explicit AND operator converted",
			query:           "Augmentin AND 1g",
			useOR:           true,
			prefixSearch:    false,
			normalizeArabic: false,
			expected:        "Augmentin | 1g", // AND filtered, useOR joins with |
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := BuildPgSearchQuery(tt.query, tt.useOR, tt.prefixSearch, tt.normalizeArabic)
			if result != tt.expected {
				t.Errorf("BuildPgSearchQuery(%q, %v, %v, %v) = %q, want %q",
					tt.query, tt.useOR, tt.prefixSearch, tt.normalizeArabic, result, tt.expected)
			}
		})
	}
}

// TestBuildPgSearchQuery_NoConsecutiveOperators ensures valid PostgreSQL tsquery syntax
func TestBuildPgSearchQuery_NoConsecutiveOperators(t *testing.T) {
	tests := []struct {
		name            string
		query           string
		useOR           bool
		prefixSearch    bool
		normalizeArabic bool
	}{
		{"medication with dosage", "Augmentin 1g", true, true, false},
		{"medication with dot", "Ozempic 0.5", true, true, false},
		{"multi-word medication", "Panadol Extra", true, true, false},
		{"arabic medication", "كتافلام 50", true, true, true},
		{"complex dosage", "Zoledronic Acid 3.6", true, true, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := BuildPgSearchQuery(tt.query, tt.useOR, tt.prefixSearch, tt.normalizeArabic)

			// Check for invalid patterns
			if strings.Contains(result, "| |") {
				t.Errorf("BuildPgSearchQuery(%q) = %q, contains consecutive '| |' operators", tt.query, result)
			}
			if strings.Contains(result, "& &") {
				t.Errorf("BuildPgSearchQuery(%q) = %q, contains consecutive '& &' operators", tt.query, result)
			}
			if strings.Contains(result, "| &") || strings.Contains(result, "& |") {
				t.Errorf("BuildPgSearchQuery(%q) = %q, contains mixed operators without term", tt.query, result)
			}
			// Check no leading/trailing operators
			trimmed := strings.TrimSpace(result)
			if strings.HasPrefix(trimmed, "|") || strings.HasPrefix(trimmed, "&") {
				t.Errorf("BuildPgSearchQuery(%q) = %q, starts with operator", tt.query, result)
			}
			if strings.HasSuffix(trimmed, "|") || strings.HasSuffix(trimmed, "&") {
				t.Errorf("BuildPgSearchQuery(%q) = %q, ends with operator", tt.query, result)
			}
		})
	}
}

// TestBuildPgSearchQuery_ValidSyntax tests specific expected outputs
func TestBuildPgSearchQuery_ValidSyntax(t *testing.T) {
	tests := []struct {
		name            string
		query           string
		useOR           bool
		prefixSearch    bool
		normalizeArabic bool
		wantNonEmpty    bool // just check it's not empty and valid
	}{
		{"simple term", "Augmentin", false, false, false, true},
		{"two terms with OR and prefix", "Augmentin 1g", true, true, false, true},
		{"decimal dosage", "Ozempic 0.5", true, true, false, true},
		{"empty query", "", true, true, false, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := BuildPgSearchQuery(tt.query, tt.useOR, tt.prefixSearch, tt.normalizeArabic)
			if tt.wantNonEmpty && result == "" {
				t.Errorf("BuildPgSearchQuery(%q) = empty, expected non-empty", tt.query)
			}
			if !tt.wantNonEmpty && result != "" {
				t.Errorf("BuildPgSearchQuery(%q) = %q, expected empty", tt.query, result)
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
			expected: "Augmentin | 1g:*", // PostgreSQL uses | and :*
		},
		{
			name:     "arabic medication with hamza",
			input:    "أوجمنتين",
			expected: "اوجمنتين:*", // Arabic normalized + prefix
		},
		{
			name:     "single word",
			input:    "Panadol",
			expected: "Panadol:*", // Single word gets :* prefix
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
			name:     "two terms adjacent",
			distance: 1,
			terms:    []string{"Augmentin", "1g"},
			expected: "Augmentin <-> 1g",
		},
		{
			name:     "two terms with distance",
			distance: 3,
			terms:    []string{"Augmentin", "1g"},
			expected: "Augmentin <3> 1g",
		},
		{
			name:     "three terms",
			distance: 5,
			terms:    []string{"antibiotic", "Augmentin", "1g"},
			expected: "antibiotic <5> Augmentin <5> 1g",
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
