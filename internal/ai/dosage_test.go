package ai

import (
	"math"
	"testing"
)

func TestParseDosage(t *testing.T) {
	tests := []struct {
		name          string
		input         string
		expectedValue float64
		expectedUnit  string
		shouldBeNil   bool
	}{
		// Basic patterns
		{"simple mg", "100mg", 100, "mg", false},
		{"simple g", "0.5g", 0.5, "g", false},
		{"simple mcg", "500mcg", 500, "mcg", false},
		{"simple ml", "10ml", 10, "ml", false},
		{"simple iu", "50iu", 50, "iu", false},

		// With spaces
		{"mg with space", "100 mg", 100, "mg", false},
		{"g with space", "0.5 g", 0.5, "g", false},

		// Decimal values
		{"decimal mg", "2.5mg", 2.5, "mg", false},
		{"decimal g", "0.25g", 0.25, "g", false},

		// In medication names
		{"Ozempic with dosage", "Ozempic 2mg", 2, "mg", false},
		{"Concor with dosage", "Concor 5mg", 5, "mg", false},
		{"Complex name", "Augmentin 1g", 1, "g", false},

		// Unit variations
		{"microgram symbol", "500μg", 500, "mcg", false},
		{"microgram ug", "500ug", 500, "mcg", false},
		{"IU variant", "100ui", 100, "iu", false},

		// Per ml notations
		{"mg per ml", "10mg/ml", 10, "mg", false},
		{"g per ml", "2g/ml", 2, "g", false},

		// Case insensitive
		{"uppercase MG", "100MG", 100, "mg", false},
		{"mixed case", "50Mg", 50, "mg", false},

		// Arabic numerals (٠-٩)
		{"arabic numerals mg", "١٠٠ملغ", 100, "mg", false},
		{"arabic numerals g", "٥٠٠جرام", 500, "g", false},
		{"arabic decimal", "٢.٥ملغ", 2.5, "mg", false},

		// Arabic unit names
		{"arabic mg variant 1", "100ملغ", 100, "mg", false},
		{"arabic mg variant 2", "100ملجم", 100, "mg", false},
		{"arabic gram", "1جرام", 1, "g", false},
		{"arabic gram variant", "1غرام", 1, "g", false},
		{"arabic ml", "10مل", 10, "ml", false},
		{"arabic microgram", "500ميكروغرام", 500, "mcg", false},

		// Mixed Arabic text with dosage
		{"medication with arabic dosage", "اوزمبك ٢ملغ", 2, "mg", false},
		{"arabic name and unit", "كونكور ٥ملغ", 5, "mg", false},

		// Edge cases - no dosage
		{"no dosage", "Ozempic", 0, "", true},
		{"empty string", "", 0, "", true},
		{"just text", "medication name", 0, "", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ParseDosage(tt.input)

			if tt.shouldBeNil {
				if result != nil {
					t.Errorf("ParseDosage(%q) = %+v, want nil", tt.input, result)
				}
				return
			}

			if result == nil {
				t.Fatalf("ParseDosage(%q) = nil, want non-nil", tt.input)
			}

			if result.Value != tt.expectedValue {
				t.Errorf("ParseDosage(%q).Value = %v, want %v", tt.input, result.Value, tt.expectedValue)
			}

			if result.Unit != tt.expectedUnit {
				t.Errorf("ParseDosage(%q).Unit = %q, want %q", tt.input, result.Unit, tt.expectedUnit)
			}
		})
	}
}

func TestDosage_ToBaseUnit(t *testing.T) {
	tests := []struct {
		name     string
		dosage   *Dosage
		expected float64
	}{
		{"100mg to mg", &Dosage{100, "mg"}, 100},
		{"1g to mg", &Dosage{1, "g"}, 1000},
		{"0.5g to mg", &Dosage{0.5, "g"}, 500},
		{"500mcg to mg", &Dosage{500, "mcg"}, 0.5},
		{"1000mcg to mg", &Dosage{1000, "mcg"}, 1},
		{"10ml to mg", &Dosage{10, "ml"}, 10}, // 1:1 approximation
		{"50iu to mg", &Dosage{50, "iu"}, 50}, // 1:1 approximation
		{"nil dosage", nil, 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := tt.dosage.ToBaseUnit()
			if math.Abs(result-tt.expected) > 0.001 {
				t.Errorf("ToBaseUnit() = %v, want %v", result, tt.expected)
			}
		})
	}
}

func TestCompareDosages(t *testing.T) {
	tests := []struct {
		name     string
		dosageA  *Dosage
		dosageB  *Dosage
		expected float64
		minScore float64 // For range testing
	}{
		// Exact matches
		{"exact same mg", &Dosage{100, "mg"}, &Dosage{100, "mg"}, 1.0, 1.0},
		{"exact same g", &Dosage{0.5, "g"}, &Dosage{0.5, "g"}, 1.0, 1.0},

		// Different units, same value
		{"1g vs 1000mg", &Dosage{1, "g"}, &Dosage{1000, "mg"}, 1.0, 1.0},
		{"500mcg vs 0.5mg", &Dosage{500, "mcg"}, &Dosage{0.5, "mg"}, 1.0, 1.0},
		{"0.5g vs 500mg", &Dosage{0.5, "g"}, &Dosage{500, "mg"}, 1.0, 1.0},

		// Within 10% tolerance (should be 1.0)
		{"105mg vs 100mg", &Dosage{105, "mg"}, &Dosage{100, "mg"}, 1.0, 1.0},
		{"95mg vs 100mg", &Dosage{95, "mg"}, &Dosage{100, "mg"}, 1.0, 1.0},

		// Moderate difference (20%)
		{"120mg vs 100mg", &Dosage{120, "mg"}, &Dosage{100, "mg"}, 0.64, 0.6},

		// Large difference (50%)
		{"150mg vs 100mg", &Dosage{150, "mg"}, &Dosage{100, "mg"}, 0.2, 0.0},

		// Very different
		{"1000mg vs 100mg", &Dosage{1000, "mg"}, &Dosage{100, "mg"}, 0.0, 0.0},

		// Nil cases
		{"nil vs valid", nil, &Dosage{100, "mg"}, 0.0, 0.0},
		{"valid vs nil", &Dosage{100, "mg"}, nil, 0.0, 0.0},
		{"nil vs nil", nil, nil, 0.0, 0.0},

		// Zero values
		{"0 vs 0", &Dosage{0, "mg"}, &Dosage{0, "mg"}, 1.0, 1.0},
		{"0 vs 100", &Dosage{0, "mg"}, &Dosage{100, "mg"}, 0.0, 0.0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := CompareDosages(tt.dosageA, tt.dosageB)

			// For exact matches, check precise value
			if tt.expected == 1.0 && result != 1.0 {
				t.Errorf("CompareDosages() = %v, want exactly 1.0", result)
			}

			// For other cases, check it's in acceptable range
			if result < tt.minScore || result > tt.expected+0.1 {
				t.Errorf("CompareDosages() = %v, want between %v and %v", result, tt.minScore, tt.expected+0.1)
			}
		})
	}
}

func TestIsSameDosage(t *testing.T) {
	tests := []struct {
		name     string
		dosageA  *Dosage
		dosageB  *Dosage
		expected bool
	}{
		{"exact match", &Dosage{100, "mg"}, &Dosage{100, "mg"}, true},
		{"within tolerance", &Dosage{105, "mg"}, &Dosage{100, "mg"}, true},
		{"unit conversion match", &Dosage{1, "g"}, &Dosage{1000, "mg"}, true},
		{"outside tolerance", &Dosage{120, "mg"}, &Dosage{100, "mg"}, false},
		{"very different", &Dosage{1000, "mg"}, &Dosage{100, "mg"}, false},
		{"nil vs valid", nil, &Dosage{100, "mg"}, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := IsSameDosage(tt.dosageA, tt.dosageB)
			if result != tt.expected {
				t.Errorf("IsSameDosage() = %v, want %v", result, tt.expected)
			}
		})
	}
}

func TestDosage_String(t *testing.T) {
	tests := []struct {
		name     string
		dosage   *Dosage
		expected string
	}{
		{"100mg", &Dosage{100, "mg"}, "100mg"},
		{"0.5g", &Dosage{0.5, "g"}, "0.5g"},
		{"2.5mg", &Dosage{2.5, "mg"}, "2.5mg"},
		{"nil", nil, "no dosage"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := tt.dosage.String()
			if result != tt.expected {
				t.Errorf("String() = %q, want %q", result, tt.expected)
			}
		})
	}
}

func TestNormalizeUnit(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		// English units
		{"mcg lowercase", "mcg", "mcg"},
		{"mcg uppercase", "MCG", "mcg"},
		{"μg symbol", "μg", "mcg"},
		{"ug variant", "ug", "mcg"},
		{"mg standard", "mg", "mg"},
		{"g standard", "g", "g"},
		{"ui variant", "ui", "iu"},
		{"iu standard", "iu", "iu"},
		{"ml standard", "ml", "ml"},

		// Arabic units
		{"arabic mg variant 1", "ملغ", "mg"},
		{"arabic mg variant 2", "ملجم", "mg"},
		{"arabic gram", "جرام", "g"},
		{"arabic gram variant", "غرام", "g"},
		{"arabic g short", "ج", "g"},
		{"arabic ml", "مل", "ml"},
		{"arabic microgram 1", "ميكروغرام", "mcg"},
		{"arabic microgram 2", "مايكروجرام", "mcg"},
		{"arabic iu", "وحدة", "iu"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := normalizeUnit(tt.input)
			if result != tt.expected {
				t.Errorf("normalizeUnit(%q) = %q, want %q", tt.input, result, tt.expected)
			}
		})
	}
}

func TestConvertArabicNumerals(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{"western numerals unchanged", "123.45", "123.45"},
		{"arabic zero", "٠", "0"},
		{"arabic single digit", "٥", "5"},
		{"arabic double digit", "١٠", "10"},
		{"arabic triple digit", "١٠٠", "100"},
		{"arabic decimal", "٢.٥", "2.5"},
		{"arabic with comma", "٣,١٤", "3,14"},
		{"all arabic digits", "٠١٢٣٤٥٦٧٨٩", "0123456789"},
		{"mixed content", "abc١٢٣def", "abc123def"},
		{"empty string", "", ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := convertArabicNumerals(tt.input)
			if result != tt.expected {
				t.Errorf("convertArabicNumerals(%q) = %q, want %q", tt.input, result, tt.expected)
			}
		})
	}
}
