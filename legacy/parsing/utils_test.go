package parsing

import (
	"testing"
)

func TestFuzzyMatch(t *testing.T) {
	tests := []struct {
		name     string
		s1       string
		s2       string
		expected float64
		delta    float64
	}{
		{
			name:     "identical strings",
			s1:       "paracetamol",
			s2:       "paracetamol",
			expected: 1.0,
			delta:    0.01,
		},
		{
			name:     "case insensitive match",
			s1:       "Paracetamol",
			s2:       "paracetamol",
			expected: 1.0,
			delta:    0.01,
		},
		{
			name:     "similar strings",
			s1:       "paracetamol",
			s2:       "paracetamole", // typo
			expected: 0.9,
			delta:    0.1,
		},
		{
			name:     "different strings",
			s1:       "aspirin",
			s2:       "ibuprofen",
			expected: 0.3,
			delta:    0.2,
		},
		{
			name:     "empty first string",
			s1:       "",
			s2:       "test",
			expected: 0.0,
			delta:    0.01,
		},
		{
			name:     "empty second string",
			s1:       "test",
			s2:       "",
			expected: 0.0,
			delta:    0.01,
		},
		{
			name:     "both empty",
			s1:       "",
			s2:       "",
			expected: 1.0,
			delta:    0.01,
		},
		{
			name:     "whitespace handling",
			s1:       "  test  ",
			s2:       "test",
			expected: 1.0,
			delta:    0.01,
		},
		{
			name:     "single character difference",
			s1:       "test",
			s2:       "tast",
			expected: 0.75,
			delta:    0.1,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := fuzzyMatch(tt.s1, tt.s2)
			if result < tt.expected-tt.delta || result > tt.expected+tt.delta {
				t.Errorf("fuzzyMatch(%q, %q) = %.4f, want %.4f (±%.2f)",
					tt.s1, tt.s2, result, tt.expected, tt.delta)
			}
		})
	}
}

func TestSimpleSubstringMatch(t *testing.T) {
	tests := []struct {
		name     string
		s1       string
		s2       string
		expected float64
		delta    float64
	}{
		{
			name:     "s1 contains s2",
			s1:       "paracetamol 500mg",
			s2:       "paracetamol",
			expected: 0.6,
			delta:    0.2,
		},
		{
			name:     "s2 contains s1",
			s1:       "aspirin",
			s2:       "aspirin extra strength",
			expected: 0.3,
			delta:    0.1,
		},
		{
			name:     "no substring match",
			s1:       "aspirin",
			s2:       "ibuprofen",
			expected: 0.0,
			delta:    0.01,
		},
		{
			name:     "identical strings",
			s1:       "test",
			s2:       "test",
			expected: 1.0,
			delta:    0.01,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := simpleSubstringMatch(tt.s1, tt.s2)
			if result < tt.expected-tt.delta || result > tt.expected+tt.delta {
				t.Errorf("simpleSubstringMatch(%q, %q) = %.4f, want %.4f (±%.2f)",
					tt.s1, tt.s2, result, tt.expected, tt.delta)
			}
		})
	}
}

func TestLevenshteinDistance(t *testing.T) {
	tests := []struct {
		name     string
		s1       string
		s2       string
		expected int
	}{
		{"identical", "test", "test", 0},
		{"one insertion", "test", "tests", 1},
		{"one deletion", "tests", "test", 1},
		{"one substitution", "test", "tist", 1},
		{"empty first", "", "test", 4},
		{"empty second", "test", "", 4},
		{"both empty", "", "", 0},
		{"completely different", "abc", "xyz", 3},
		{"partial match", "kitten", "sitting", 3},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := levenshteinDistance(tt.s1, tt.s2)
			if result != tt.expected {
				t.Errorf("levenshteinDistance(%q, %q) = %d, want %d",
					tt.s1, tt.s2, result, tt.expected)
			}
		})
	}
}

func TestGenerateTrigrams_Utils(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected []string
	}{
		{
			name:     "normal string",
			input:    "hello",
			expected: []string{"hel", "ell", "llo"},
		},
		{
			name:     "exactly 3 chars",
			input:    "abc",
			expected: []string{"abc"},
		},
		{
			name:     "less than 3 chars",
			input:    "ab",
			expected: nil,
		},
		{
			name:     "empty string",
			input:    "",
			expected: nil,
		},
		{
			name:     "unicode string",
			input:    "مرحبا", // Arabic "hello"
			expected: []string{"مرح", "رحب", "حبا"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := generateTrigrams(tt.input)
			if len(result) != len(tt.expected) {
				t.Errorf("generateTrigrams(%q) = %v, want %v", tt.input, result, tt.expected)
				return
			}
			for i, tri := range result {
				if tri != tt.expected[i] {
					t.Errorf("generateTrigrams(%q)[%d] = %q, want %q",
						tt.input, i, tri, tt.expected[i])
				}
			}
		})
	}
}

func TestSanitizeForFTS(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "simple words",
			input:    "hello world",
			expected: "hello OR world",
		},
		{
			name:     "with quotes",
			input:    `"hello" 'world'`,
			expected: "hello OR world",
		},
		{
			name:     "with special chars",
			input:    "hello-world (test)",
			expected: "hello OR world OR test",
		},
		{
			name:     "with wildcards",
			input:    "hello* world",
			expected: "hello OR world",
		},
		{
			name:     "empty string",
			input:    "",
			expected: "",
		},
		{
			name:     "only special chars",
			input:    "()-*'\"",
			expected: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := sanitizeForFTS(tt.input)
			if result != tt.expected {
				t.Errorf("sanitizeForFTS(%q) = %q, want %q", tt.input, result, tt.expected)
			}
		})
	}
}

func TestMin(t *testing.T) {
	tests := []struct {
		a, b, c  int
		expected int
	}{
		{1, 2, 3, 1},
		{3, 2, 1, 1},
		{2, 1, 3, 1},
		{5, 5, 5, 5},
		{0, 1, 2, 0},
		{-1, 0, 1, -1},
	}

	for _, tt := range tests {
		result := min(tt.a, tt.b, tt.c)
		if result != tt.expected {
			t.Errorf("min(%d, %d, %d) = %d, want %d", tt.a, tt.b, tt.c, result, tt.expected)
		}
	}
}

func TestMax(t *testing.T) {
	tests := []struct {
		a, b     int
		expected int
	}{
		{1, 2, 2},
		{2, 1, 2},
		{5, 5, 5},
		{0, 1, 1},
		{-1, 0, 0},
	}

	for _, tt := range tests {
		result := max(tt.a, tt.b)
		if result != tt.expected {
			t.Errorf("max(%d, %d) = %d, want %d", tt.a, tt.b, result, tt.expected)
		}
	}
}

// Benchmark tests
func BenchmarkFuzzyMatch_Short(b *testing.B) {
	for i := 0; i < b.N; i++ {
		fuzzyMatch("paracetamol", "paracetamole")
	}
}

func BenchmarkFuzzyMatch_Long(b *testing.B) {
	longStr1 := "this is a very long medication name with many words"
	longStr2 := "this is a very long medication name with some words"
	for i := 0; i < b.N; i++ {
		fuzzyMatch(longStr1, longStr2)
	}
}

func BenchmarkLevenshteinDistance(b *testing.B) {
	for i := 0; i < b.N; i++ {
		levenshteinDistance("kitten", "sitting")
	}
}

func BenchmarkGenerateTrigrams(b *testing.B) {
	for i := 0; i < b.N; i++ {
		generateTrigrams("paracetamol")
	}
}
