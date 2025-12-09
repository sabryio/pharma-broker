package dosage

import (
	"math"
	"regexp"
	"strconv"
	"strings"
)

// Dosage represents a medication dosage with value and unit
type Dosage struct {
	Value float64 // Numeric dosage value
	Unit  string  // Unit: "mg", "g", "mcg", "ml", "iu", etc.
}

// Unit conversion constants (all to mg as base unit)
const (
	MgPerG   = 1000.0 // 1 gram = 1000 milligrams
	McGPerMg = 1000.0 // 1 milligram = 1000 micrograms
	IUPerMg  = 1.0    // IU conversion is medication-specific, use 1:1 as default
	MlPerMg  = 1.0    // ml to mg is density-dependent, use 1:1 as default
)

// Dosage pattern regex
// Matches patterns like: "100mg", "0.5g", "500 mcg", "2mg/ml", etc.
// Also matches Arabic numerals: "١٠٠ملغ", "٥٠٠ميكروغرام"
// Longer units are listed first to avoid partial matches
var dosagePattern = regexp.MustCompile(`(?i)([\d٠-٩]+(?:[\.,]?[\d٠-٩]+)?)\s*(ميكروغرام|مايكروجرام|جرام|غرام|ملغ|ملجم|وحدة|mcg|μg|ug|mg|ml|iu|ui|مل|ج|g)(?:/\w+)?`)

// Arabic numeral to Western numeral mapping
var arabicNumerals = map[rune]rune{
	'٠': '0', '١': '1', '٢': '2', '٣': '3', '٤': '4',
	'٥': '5', '٦': '6', '٧': '7', '٨': '8', '٩': '9',
}

// ParseDosage extracts dosage information from a medication string
// Supports both Western and Arabic numerals and unit names
// Returns nil if no dosage found
func ParseDosage(medication string) *Dosage {
	if medication == "" {
		return nil
	}

	matches := dosagePattern.FindStringSubmatch(medication)
	if len(matches) < 3 {
		return nil // No dosage found
	}

	// Convert Arabic numerals to Western if needed
	valueStr := convertArabicNumerals(matches[1])

	// Extract value
	value, err := strconv.ParseFloat(valueStr, 64)
	if err != nil {
		return nil
	}

	// Normalize unit (handles both English and Arabic unit names)
	unit := normalizeUnit(matches[2])

	return &Dosage{
		Value: value,
		Unit:  unit,
	}
}

// convertArabicNumerals converts Arabic numerals (٠-٩) to Western (0-9)
func convertArabicNumerals(s string) string {
	var result strings.Builder
	for _, r := range s {
		if western, ok := arabicNumerals[r]; ok {
			result.WriteRune(western)
		} else {
			result.WriteRune(r)
		}
	}
	return result.String()
}

// normalizeUnit standardizes unit representation (supports Arabic and English)
func normalizeUnit(unit string) string {
	unit = strings.ToLower(strings.TrimSpace(unit))

	// Map variations to standard units
	switch unit {
	// English variations
	case "μg", "ug":
		return "mcg" // Microgram
	case "ui":
		return "iu" // International Unit

	// Arabic unit names
	case "ملغ", "ملجم": // milligram
		return "mg"
	case "جرام", "غرام", "ج": // gram
		return "g"
	case "ميكروغرام", "مايكروجرام": // microgram
		return "mcg"
	case "مل": // milliliter
		return "ml"
	case "وحدة", "وحدة دولية": // international unit
		return "iu"

	default:
		return unit
	}
}

// ToBaseUnit converts the dosage to base unit (mg)
// This allows for normalized comparison across different units
func (d *Dosage) ToBaseUnit() float64 {
	if d == nil {
		return 0
	}

	switch d.Unit {
	case "g":
		return d.Value * MgPerG // grams to mg
	case "mcg":
		return d.Value / McGPerMg // micrograms to mg
	case "mg":
		return d.Value // already in mg
	case "ml":
		return d.Value * MlPerMg // ml to mg (approximate)
	case "iu":
		return d.Value * IUPerMg // IU to mg (approximate)
	default:
		return d.Value // Unknown unit, return as-is
	}
}

// CompareDosages calculates similarity between two dosages (0.0-1.0)
// Returns 1.0 for exact match, decreasing as difference increases
// Returns 0.0 if either dosage is nil
func CompareDosages(a, b *Dosage) float64 {
	if a == nil || b == nil {
		return 0.0
	}

	// Normalize both to base unit (mg)
	aBase := a.ToBaseUnit()
	bBase := b.ToBaseUnit()

	// Handle zero values
	if aBase == 0 && bBase == 0 {
		return 1.0
	}
	if aBase == 0 || bBase == 0 {
		return 0.0
	}

	// Calculate percentage difference
	diff := math.Abs(aBase - bBase)
	avg := (aBase + bBase) / 2.0
	percentDiff := diff / avg

	// Convert to similarity score (1.0 = exact, 0.0 = very different)
	// Allow 10% tolerance for "perfect" match
	if percentDiff <= 0.1 {
		return 1.0
	}

	// Linear decay from 1.0 to 0.0 as difference grows
	// 50% difference = 0.5 score, 100% difference = 0.0 score
	similarity := math.Max(0, 1.0-(percentDiff*2.0))

	return similarity
}

// IsSameDosage returns true if dosages are equivalent (within 10% tolerance)
func IsSameDosage(a, b *Dosage) bool {
	return CompareDosages(a, b) >= 0.9
}

// String returns human-readable dosage representation
func (d *Dosage) String() string {
	if d == nil {
		return "no dosage"
	}
	return strconv.FormatFloat(d.Value, 'f', -1, 64) + d.Unit
}
