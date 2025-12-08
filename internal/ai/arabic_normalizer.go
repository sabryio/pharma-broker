package ai

import (
	"slices"
	"strings"
	"unicode"
)

// Arabic diacritics (tashkeel) to remove
var arabicDiacritics = []rune{
	'\u064B', // Fathatan
	'\u064C', // Dammatan
	'\u064D', // Kasratan
	'\u064E', // Fatha
	'\u064F', // Damma
	'\u0650', // Kasra
	'\u0651', // Shadda
	'\u0652', // Sukun
	'\u0670', // Superscript Alef
}

// Alef variants to normalize to plain Alef (ا)
var alefVariants = map[rune]rune{
	'أ': 'ا', // Alef with Hamza Above
	'إ': 'ا', // Alef with Hamza Below
	'آ': 'ا', // Alef with Madda
	'ٱ': 'ا', // Alef Wasla
}

// NormalizeArabic normalizes Arabic text for better matching:
// - Removes diacritics (tashkeel)
// - Normalizes Alef variants (أ إ آ ٱ → ا)
// - Normalizes Taa Marbuta (ة → ه)
// - Normalizes Alef Maksura (ى → ي)
// - Removes Tatweel (ـ)
func NormalizeArabic(text string) string {
	var result strings.Builder
	result.Grow(len(text))

	for _, r := range text {
		// Skip diacritics
		if isDiacritic(r) {
			continue
		}

		// Skip Tatweel
		if r == 'ـ' {
			continue
		}

		// Normalize Alef variants
		if normalized, ok := alefVariants[r]; ok {
			result.WriteRune(normalized)
			continue
		}

		// Normalize Taa Marbuta
		if r == 'ة' {
			result.WriteRune('ه')
			continue
		}

		// Normalize Alef Maksura
		if r == 'ى' {
			result.WriteRune('ي')
			continue
		}

		result.WriteRune(r)
	}

	return result.String()
}

// isDiacritic checks if a rune is an Arabic diacritic
func isDiacritic(r rune) bool {
	if slices.Contains(arabicDiacritics, r) {
		return true
	}
	// Also check Unicode category for combining marks
	return unicode.Is(unicode.Mn, r) // Mn = Mark, Nonspacing
}

// NormalizeForMatching normalizes text for medication matching
// Applies Arabic normalization and lowercases
func NormalizeForMatching(text string) string {
	normalized := NormalizeArabic(text)
	return strings.ToLower(normalized)
}
