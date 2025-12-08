package ai

import (
	"testing"
)

func TestNormalizeArabic(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		// Alef variants
		{"hamza above", "أوفتريل", "اوفتريل"},
		{"hamza below", "إنسولين", "انسولين"},
		{"madda", "آلام", "الام"},
		{"alef wasla", "ٱلرحمن", "الرحمن"},

		// Taa marbuta
		{"taa marbuta", "صيدلية", "صيدليه"},
		{"taa marbuta middle", "صيدليةالمدينة", "صيدليهالمدينه"},

		// Alef maksura
		{"alef maksura", "مستشفى", "مستشفي"},
		{"alef maksura end", "موسى", "موسي"},

		// Tatweel (kashida)
		{"tatweel", "مــريض", "مريض"},

		// Diacritics
		{"fatha", "مَريض", "مريض"},
		{"kasra", "مِريض", "مريض"},
		{"damma", "مُريض", "مريض"},
		{"shadda", "مرَّض", "مرض"},
		{"sukun", "مْريض", "مريض"},

		// Combined
		{"complex", "أَوْزِمْبِك", "اوزمبك"},

		// English/numbers passthrough
		{"english", "Ozempic", "Ozempic"},
		{"mixed", "أوزمبك 100mg", "اوزمبك 100mg"},
		{"numbers", "150", "150"},

		// Empty
		{"empty", "", ""},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			result := NormalizeArabic(tc.input)
			if result != tc.expected {
				t.Errorf("NormalizeArabic(%q) = %q, want %q", tc.input, result, tc.expected)
			}
		})
	}
}

func TestNormalizeForMatching(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{"arabic with hamza", "أوفتريل", "اوفتريل"},
		{"english uppercase", "OZEMPIC", "ozempic"},
		{"mixed case", "Ozempic", "ozempic"},
		{"arabic unchanged", "اوزمبك", "اوزمبك"},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			result := NormalizeForMatching(tc.input)
			if result != tc.expected {
				t.Errorf("NormalizeForMatching(%q) = %q, want %q", tc.input, result, tc.expected)
			}
		})
	}
}

func TestIsDiacritic(t *testing.T) {
	diacritics := []rune{'\u064E', '\u064F', '\u0650', '\u0651', '\u0652'}
	nonDiacritics := []rune{'ا', 'ب', 'A', '1', ' '}

	for _, r := range diacritics {
		if !isDiacritic(r) {
			t.Errorf("isDiacritic(%q) = false, want true", r)
		}
	}

	for _, r := range nonDiacritics {
		if isDiacritic(r) {
			t.Errorf("isDiacritic(%q) = true, want false", r)
		}
	}
}
