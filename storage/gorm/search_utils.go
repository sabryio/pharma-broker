package gorm

import (
	"regexp"
	"strings"
	"unicode"
)

// Common Arabic diacritics (tashkeel) to remove for better matching
var arabicDiacritics = strings.NewReplacer(
	"\u064B", "", // fathatan
	"\u064C", "", // dammatan
	"\u064D", "", // kasratan
	"\u064E", "", // fatha
	"\u064F", "", // damma
	"\u0650", "", // kasra
	"\u0651", "", // shadda
	"\u0652", "", // sukun
	"\u0670", "", // superscript alef
	"\u0640", "", // tatweel (kashida)
)

// Common Arabic letter normalizations for fuzzy matching
var arabicNormalizer = strings.NewReplacer(
	"أ", "ا", // alef with hamza above -> alef
	"إ", "ا", // alef with hamza below -> alef
	"آ", "ا", // alef with madda -> alef
	"ى", "ي", // alef maksura -> yeh
	"ة", "ه", // teh marbuta -> heh
	"ؤ", "و", // waw with hamza -> waw
	"ئ", "ي", // yeh with hamza -> yeh
)

// PostgreSQL tsquery operators
var pgOperators = map[string]string{
	"AND": "&",
	"OR":  "|",
	"NOT": "!",
}

// SanitizePgQuery sanitizes the user query for PostgreSQL tsquery.
// It converts the query into a format compatible with to_tsquery.
func SanitizePgQuery(query string) string {
	if query == "" {
		return ""
	}

	// Split by whitespace
	parts := strings.Fields(query)
	var processed []string

	for _, part := range parts {
		// Check if it's an operator
		if pgOp, ok := pgOperators[strings.ToUpper(part)]; ok {
			processed = append(processed, pgOp)
			continue
		}

		// Skip empty parts
		if part == "" {
			continue
		}

		// Remove special characters that break tsquery
		cleanPart := removeSpecialChars(part)
		if cleanPart == "" {
			continue
		}

		// For prefix search (ends with *)
		if strings.HasSuffix(part, "*") {
			term := strings.TrimSuffix(cleanPart, "*")
			if term != "" {
				processed = append(processed, term+":*")
			}
			continue
		}

		processed = append(processed, cleanPart)
	}

	// Join with OR by default for flexible matching
	return strings.Join(processed, " | ")
}

// removeSpecialChars removes characters that break PostgreSQL tsquery
func removeSpecialChars(s string) string {
	var result strings.Builder
	for _, r := range s {
		if unicode.IsLetter(r) || unicode.IsNumber(r) || r == '*' {
			result.WriteRune(r)
		}
	}
	return result.String()
}

// NormalizeArabic removes diacritics and normalizes Arabic letters for consistent matching.
// This helps match "الأوجمنتين" with "الاوجمنتين" (hamza variations).
func NormalizeArabic(text string) string {
	// Remove diacritics first
	text = arabicDiacritics.Replace(text)
	// Normalize letter forms
	text = arabicNormalizer.Replace(text)
	return text
}

// BuildPgSearchQuery creates a PostgreSQL tsquery from user input.
// Options allow customizing the search behavior:
//   - useOR: join terms with | instead of &
//   - prefixSearch: add :* to last term for prefix matching
//   - normalizeArabicText: normalize Arabic text before searching
func BuildPgSearchQuery(query string, useOR, prefixSearch, normalizeArabicText bool) string {
	if query == "" {
		return ""
	}

	// Optionally normalize Arabic
	if normalizeArabicText {
		query = NormalizeArabic(query)
	}

	// Split and process
	parts := strings.Fields(query)
	if len(parts) == 0 {
		return ""
	}

	var terms []string

	for i, part := range parts {
		// Skip empty parts
		if part == "" {
			continue
		}

		// Check if it's an operator
		if pgOp, ok := pgOperators[strings.ToUpper(part)]; ok {
			terms = append(terms, pgOp)
			continue
		}

		// Clean the term
		cleanPart := removeSpecialChars(part)
		if cleanPart == "" {
			continue
		}

		// Add prefix to last term if requested
		isLastTerm := i == len(parts)-1
		if prefixSearch && isLastTerm && !strings.HasSuffix(cleanPart, "*") {
			terms = append(terms, cleanPart+":*")
			continue
		}

		// Handle existing prefix notation
		if strings.HasSuffix(part, "*") {
			term := strings.TrimSuffix(cleanPart, "*")
			if term != "" {
				terms = append(terms, term+":*")
			}
			continue
		}

		terms = append(terms, cleanPart)
	}

	// If no valid terms, return empty
	if len(terms) == 0 {
		return ""
	}

	// If only one term, return it directly
	if len(terms) == 1 {
		return terms[0]
	}

	// Join with appropriate operator
	if useOR {
		return strings.Join(terms, " | ")
	}
	return strings.Join(terms, " & ")
}

// BuildMedicationSearchQuery creates an optimized query for medication search.
// It applies Arabic normalization and uses OR for flexible matching.
func BuildMedicationSearchQuery(medication string) string {
	if medication == "" {
		return ""
	}

	// Normalize Arabic for consistent matching
	normalized := NormalizeArabic(medication)

	// Use OR-based search with prefix on last term for partial matches
	return BuildPgSearchQuery(normalized, true, true, false)
}

// BuildPgILikePattern creates a PostgreSQL ILIKE pattern for fuzzy matching.
// Useful for simple substring searches without full-text search.
func BuildPgILikePattern(query string) string {
	if query == "" {
		return "%"
	}
	// Escape special LIKE characters
	escaped := strings.NewReplacer(
		"%", "\\%",
		"_", "\\_",
		"\\", "\\\\",
	).Replace(query)

	return "%" + escaped + "%"
}

// BuildTrigramQuery creates a query suitable for trigram (pg_trgm) similarity search.
// Returns the normalized query and minimum similarity threshold.
func BuildTrigramQuery(query string, normalizeArabicText bool) (string, float64) {
	if query == "" {
		return "", 0.3
	}

	if normalizeArabicText {
		query = NormalizeArabic(query)
	}

	// Default similarity threshold for Arabic text
	return query, 0.3
}

// HighlightMatches wraps matched terms with markers for display.
// Uses ts_headline() in actual queries, this is a fallback for manual highlighting.
func HighlightMatches(text, query, startTag, endTag string) string {
	if text == "" || query == "" {
		return text
	}

	terms := strings.Fields(strings.ToLower(query))
	result := text

	for _, term := range terms {
		// Skip operators
		if _, isOp := pgOperators[strings.ToUpper(term)]; isOp {
			continue
		}
		// Remove prefix notation
		term = strings.TrimSuffix(term, ":*")
		if term == "" {
			continue
		}

		// Case-insensitive replacement
		re, err := regexp.Compile("(?i)" + regexp.QuoteMeta(term))
		if err != nil {
			continue
		}
		result = re.ReplaceAllStringFunc(result, func(match string) string {
			return startTag + match + endTag
		})
	}

	return result
}

// containsSpecialChars checks if string has non-letter/number characters
func containsSpecialChars(s string) bool {
	for _, r := range s {
		if !unicode.IsLetter(r) && !unicode.IsNumber(r) {
			return true
		}
	}
	return false
}

// ============================================================
// DEPRECATED FUNCTIONS - Kept for backward compatibility
// These were used for SQLite FTS5 and should be replaced
// ============================================================

// Deprecated: Use SanitizePgQuery instead
func SanitizeFTSQuery(query string) string {
	return SanitizePgQuery(query)
}

// Deprecated: Use BuildPgSearchQuery instead
func BuildSearchQuery(query string, useOR, prefixSearch, normalizeArabicText bool) string {
	return BuildPgSearchQuery(query, useOR, prefixSearch, normalizeArabicText)
}

// Deprecated: BuildProximityQuery was used for FTS5 NEAR queries
// PostgreSQL uses phrase search <-> or <N> operators instead
func BuildProximityQuery(distance int, terms ...string) string {
	if len(terms) < 2 {
		if len(terms) == 1 {
			return SanitizePgQuery(terms[0])
		}
		return ""
	}

	// In PostgreSQL, use phrase search: term1 <-> term2 (adjacent)
	// or term1 <N> term2 (within N words)
	var quoted []string
	for _, term := range terms {
		clean := removeSpecialChars(term)
		if clean != "" {
			quoted = append(quoted, clean)
		}
	}

	// Use phrase search operator for proximity
	if distance <= 1 {
		return strings.Join(quoted, " <-> ")
	}
	return strings.Join(quoted, " <"+itoa(distance)+"> ")
}

// itoa converts int to string without importing strconv
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	var digits []byte
	for n > 0 {
		digits = append([]byte{byte('0' + n%10)}, digits...)
		n /= 10
	}
	return string(digits)
}
