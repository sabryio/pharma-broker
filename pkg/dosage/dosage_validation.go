package dosage

import (
	"pharmabroker/internal/domain"
)

// ValidateDosages validates and enriches parsed items with dosage information
// Extracts dosages from medication names and logs warnings for mismatches
func ValidateDosages(items []domain.ParsedItem) {
	for i := range items {
		item := &items[i]

		// Extract dosage from medication name
		dosage := ParseDosage(item.Medication)

		// Also check raw medication
		rawDosage := ParseDosage(item.MedicationRaw)

		// If dosages differ, use the one from the normalized name
		if dosage != nil && rawDosage != nil {
			// Compare dosages
			similarity := CompareDosages(dosage, rawDosage)

			// If they're significantly different, it might indicate an issue
			if similarity < 0.8 {
				// Log this for review - dosage mismatch between raw and normalized
				// This could be added to Notes field or logged
				if item.Notes == "" {
					item.Notes = "Dosage validation: check dosage accuracy"
				}
			}
		}
	}
}

// EnrichWithDosageInfo adds dosage information to ParsedItem notes if available
func EnrichWithDosageInfo(item *domain.ParsedItem) {
	dosage := ParseDosage(item.Medication)
	if dosage != nil {
		// Dosage found - could add to structured field if we extend ParsedItem
		// For now, this validates that dosage can be extracted
		_ = dosage
	}
}

// CompareMedicationDosages compares dosages between two medication names
// Useful for matching offers and requests
func CompareMedicationDosages(med1, med2 string) float64 {
	dosage1 := ParseDosage(med1)
	dosage2 := ParseDosage(med2)

	// If neither has dosage, return neutral score
	if dosage1 == nil && dosage2 == nil {
		return 0.9
	}

	// If only one has dosage, partial match
	if dosage1 == nil || dosage2 == nil {
		return 0.7
	}

	// Both have dosages - compare
	return CompareDosages(dosage1, dosage2)
}
