package storage

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestSanitizeFTSQuery(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "Empty",
			input:    "",
			expected: "",
		},
		{
			name:     "Simple word",
			input:    "Zoladex",
			expected: "\"Zoladex\"",
		},
		{
			name:     "With dot",
			input:    "3.6",
			expected: "\"3.6\"",
		},
		{
			name:     "With OR operator",
			input:    "Zoladex OR 3.6",
			expected: "\"Zoladex\" OR \"3.6\"",
		},
		{
			name:     "With AND operator",
			input:    "Zoladex AND 3.6",
			expected: "\"Zoladex\" AND \"3.6\"",
		},
		{
			name:     "With lower case operators (treated as text)",
			input:    "Zoladex or 3.6",
			expected: "\"Zoladex\" \"or\" \"3.6\"",
		},
		{
			name:     "Already quoted",
			input:    "\"Zoladex\"",
			expected: "\"Zoladex\"",
		},
		{
			name:     "With prefix wildcard",
			input:    "Zola*",
			expected: "\"Zola\"*",
		},
		{
			name:     "Complex case",
			input:    "Augmentin 1.2g - syrup",
			expected: "\"Augmentin\" \"1.2g\" \"-\" \"syrup\"",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := SanitizeFTSQuery(tt.input)
			assert.Equal(t, tt.expected, result)
		})
	}
}
