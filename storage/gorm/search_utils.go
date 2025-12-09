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

// FTS5 operators that should be preserved
var ftsOperators = map[string]bool{
	"AND":  true,
	"OR":   true,
	"NOT":  true,
	"NEAR": true,
}

// nearPattern matches NEAR/n syntax like NEAR/5
var nearPattern = regexp.MustCompile(`^NEAR/\d+$`)

// SanitizeFTSQuery sanitizes the user query for SQLite FTS5.
// It ensures that search terms are properly quoted to handle special characters
// like ".", "-", etc., while preserving FTS operators (AND, OR, NOT, NEAR/n).
func SanitizeFTSQuery(query string) string {
	if query == "" {
		return ""
	}

	// Split by whitespace
	parts := strings.Fields(query)
	var processed []string

	for _, part := range parts {
		// If it's a keyword, keep it as is (FTS operators are case-sensitive)
		if ftsOperators[part] || nearPattern.MatchString(part) {
			processed = append(processed, part)
			continue
		}

		// Check if it's already quoted
		if strings.HasPrefix(part, "\"") && strings.HasSuffix(part, "\"") {
			processed = append(processed, part)
			continue
		}

		// Check for column filter syntax (column:term)
		if idx := strings.Index(part, ":"); idx > 0 && idx < len(part)-1 {
			column := part[:idx]
			term := part[idx+1:]
			term = strings.ReplaceAll(term, "\"", "\"\"")
			if containsSpecialChars(term) {
				processed = append(processed, column+":\""+term+"\"")
			} else {
				processed = append(processed, column+":"+term)
			}
			continue
		}

		// Check for valid prefix query (ending with *)
		if strings.HasSuffix(part, "*") {
			// Quote the part before the *
			term := strings.TrimSuffix(part, "*")
			// Escape existing quotes in term
			term = strings.ReplaceAll(term, "\"", "\"\"")
			processed = append(processed, "\""+term+"\"*")
			continue
		}

		// Escape existing quotes
		cleanPart := strings.ReplaceAll(part, "\"", "\"\"")

		// If the part contains non-alphanumeric chars (like . - /), wrap in quotes
		if containsSpecialChars(cleanPart) {
			processed = append(processed, "\""+cleanPart+"\"")
		} else {
			// Simple words are better left unquoted to ensure tokenizer handles them naturally
			processed = append(processed, cleanPart)
		}
	}

	return strings.Join(processed, " ")
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

// BuildSearchQuery creates an FTS5 query from user input with smart defaults.
// Options allow customizing the search behavior:
//   - useOR: join terms with OR instead of implicit AND
//   - prefixSearch: add * to last term for prefix matching
//   - normalizeArabic: normalize Arabic text before searching
func BuildSearchQuery(query string, useOR, prefixSearch, normalizeArabic bool) string {
	if query == "" {
		return ""
	}

	// Optionally normalize Arabic
	if normalizeArabic {
		query = NormalizeArabic(query)
	}

	// Split and process
	parts := strings.Fields(query)
	if len(parts) == 0 {
		return ""
	}

	// Separate terms from operators
	var terms []string
	var hasExplicitOperator bool

	for i, part := range parts {
		// Skip empty parts
		if part == "" {
			continue
		}

		// Check if it's an operator
		if ftsOperators[part] || nearPattern.MatchString(part) {
			// If we have an explicit operator, don't use auto-OR
			hasExplicitOperator = true
			terms = append(terms, part)
			continue
		}

		// Escape quotes
		cleanPart := strings.ReplaceAll(part, "\"", "\"\"")
		if cleanPart == "" {
			continue
		}

		// Add prefix wildcard to last non-operator term if requested
		isLastTerm := true
		for j := i + 1; j < len(parts); j++ {
			if !ftsOperators[parts[j]] && !nearPattern.MatchString(parts[j]) {
				isLastTerm = false
				break
			}
		}

		if prefixSearch && isLastTerm && !strings.HasSuffix(cleanPart, "*") {
			if containsSpecialChars(cleanPart) {
				terms = append(terms, "\""+cleanPart+"\"*")
			} else {
				terms = append(terms, cleanPart+"*")
			}
			continue
		}

		// Quote if contains special chars
		if containsSpecialChars(cleanPart) {
			terms = append(terms, "\""+cleanPart+"\"")
		} else {
			terms = append(terms, cleanPart)
		}
	}

	// If no valid terms, return empty
	if len(terms) == 0 {
		return ""
	}

	// If only one term, return it directly
	if len(terms) == 1 {
		return terms[0]
	}

	// If there are explicit operators in the query, just join with spaces
	// (the operators are already in the terms slice)
	if hasExplicitOperator {
		return strings.Join(terms, " ")
	}

	// Join with appropriate operator
	if useOR {
		return strings.Join(terms, " OR ")
	}
	return strings.Join(terms, " ")
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
	return BuildSearchQuery(normalized, true, true, false)
}

// BuildProximityQuery creates a NEAR/n query for terms that should appear close together.
// Example: BuildProximityQuery(3, "Augmentin", "1g") -> "Augmentin" NEAR/3 "1g"
func BuildProximityQuery(distance int, terms ...string) string {
	if len(terms) < 2 {
		if len(terms) == 1 {
			return SanitizeFTSQuery(terms[0])
		}
		return ""
	}

	var quoted []string
	for _, term := range terms {
		term = strings.ReplaceAll(term, "\"", "\"\"")
		quoted = append(quoted, "\""+term+"\"")
	}

	return strings.Join(quoted, " NEAR/"+itoa(distance)+" ")
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

func containsSpecialChars(s string) bool {
	for _, r := range s {
		if !unicode.IsLetter(r) && !unicode.IsNumber(r) {
			return true
		}
	}
	return false
}

// HighlightMatches wraps matched terms with markers for display.
// Uses FTS5 snippet() or highlight() functions in the actual query,
// this is a fallback for manual highlighting.
func HighlightMatches(text, query, startTag, endTag string) string {
	if text == "" || query == "" {
		return text
	}

	terms := strings.Fields(strings.ToLower(query))
	result := text

	for _, term := range terms {
		// Skip operators
		if ftsOperators[strings.ToUpper(term)] {
			continue
		}
		// Remove quotes and wildcards
		term = strings.Trim(term, "\"*")
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
