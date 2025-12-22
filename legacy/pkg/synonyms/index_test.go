package synonyms

import (
	"testing"

	"pharmabroker/domain/entity"
)

func TestNewSynonymIndex(t *testing.T) {
	mappings := []*entity.MedicationMapping{
		{
			EnglishName: "Ozempic",
			ArabicName:  "اوزمبك",
			Synonyms:    []string{"Semaglutide", "GLP-1 agonist"},
		},
		{
			EnglishName: "Zoladex",
			ArabicName:  "زولادكس",
			Synonyms:    []string{"Goserelin"},
		},
	}

	idx := NewSynonymIndex(mappings)

	// Verify size
	if idx.Size() != 2 {
		t.Errorf("Size() = %d, want 2", idx.Size())
	}

	// Verify total mappings (Ozempic: 4 names, Zoladex: 3 names = 7)
	if idx.TotalMappings() < 7 {
		t.Errorf("TotalMappings() = %d, want >= 7", idx.TotalMappings())
	}
}

func TestSynonymIndex_GetCanonical(t *testing.T) {
	mappings := []*entity.MedicationMapping{
		{
			EnglishName: "Ozempic",
			ArabicName:  "اوزمبك",
			Synonyms:    []string{"Semaglutide", "GLP-1 agonist"},
		},
	}
	idx := NewSynonymIndex(mappings)

	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{"english name exact", "Ozempic", "Ozempic"},
		{"english name lowercase", "ozempic", "Ozempic"},
		{"english name uppercase", "OZEMPIC", "Ozempic"},
		{"arabic name", "اوزمبك", "Ozempic"},
		{"synonym", "Semaglutide", "Ozempic"},
		{"synonym lowercase", "semaglutide", "Ozempic"},
		{"unknown", "Unknown Drug", ""},
		{"empty", "", ""},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			result := idx.GetCanonical(tc.input)
			if result != tc.expected {
				t.Errorf("GetCanonical(%q) = %q, want %q", tc.input, result, tc.expected)
			}
		})
	}
}

func TestSynonymIndex_GetAllSynonyms(t *testing.T) {
	mappings := []*entity.MedicationMapping{
		{
			EnglishName: "Ozempic",
			ArabicName:  "اوزمبك",
			Synonyms:    []string{"Semaglutide"},
		},
	}
	idx := NewSynonymIndex(mappings)

	// Known medication - should return all synonyms
	syns := idx.GetAllSynonyms("Ozempic")
	if len(syns) != 3 { // Ozempic, اوزمبك, Semaglutide
		t.Errorf("GetAllSynonyms(Ozempic) returned %d items, want 3", len(syns))
	}

	// Same result for synonym lookup
	syns = idx.GetAllSynonyms("semaglutide")
	if len(syns) != 3 {
		t.Errorf("GetAllSynonyms(semaglutide) returned %d items, want 3", len(syns))
	}

	// Unknown medication - should return input only
	syns = idx.GetAllSynonyms("UnknownDrug")
	if len(syns) != 1 || syns[0] != "UnknownDrug" {
		t.Errorf("GetAllSynonyms(UnknownDrug) = %v, want [UnknownDrug]", syns)
	}
}

func TestSynonymIndex_AreSynonyms(t *testing.T) {
	mappings := []*entity.MedicationMapping{
		{
			EnglishName: "Ozempic",
			ArabicName:  "اوزمبك",
			Synonyms:    []string{"Semaglutide"},
		},
		{
			EnglishName: "Zoladex",
			ArabicName:  "زولادكس",
		},
	}
	idx := NewSynonymIndex(mappings)

	tests := []struct {
		name     string
		name1    string
		name2    string
		expected bool
	}{
		{"same exact", "Ozempic", "Ozempic", true},
		{"case insensitive", "ozempic", "OZEMPIC", true},
		{"english to arabic", "Ozempic", "اوزمبك", true},
		{"synonym match", "Ozempic", "Semaglutide", true},
		{"synonym to arabic", "Semaglutide", "اوزمبك", true},
		{"different drugs", "Ozempic", "Zoladex", false},
		{"unknown same", "Unknown", "Unknown", true},
		{"unknown different", "Unknown1", "Unknown2", false},
		{"known vs unknown", "Ozempic", "Unknown", false},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			result := idx.AreSynonyms(tc.name1, tc.name2)
			if result != tc.expected {
				t.Errorf("AreSynonyms(%q, %q) = %v, want %v", tc.name1, tc.name2, result, tc.expected)
			}
		})
	}
}

func TestSynonymIndex_NilSafe(t *testing.T) {
	var idx *SynonymIndex

	// All methods should be nil-safe
	if idx.GetCanonical("test") != "" {
		t.Error("GetCanonical on nil should return empty")
	}

	syns := idx.GetAllSynonyms("test")
	if len(syns) != 1 || syns[0] != "test" {
		t.Error("GetAllSynonyms on nil should return input")
	}

	if !idx.AreSynonyms("test", "test") {
		t.Error("AreSynonyms on nil should do case-insensitive compare")
	}

	if idx.Size() != 0 {
		t.Error("Size on nil should return 0")
	}
}
