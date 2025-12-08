package ai

import (
	"strings"

	"pharmabroker/internal/domain"
)

// SynonymIndex provides efficient synonym lookup for medication matching.
// It maps normalized medication names to their canonical English name.
type SynonymIndex struct {
	// Maps normalized name (lowercase) -> canonical English name
	toCanonical map[string]string
	// Maps canonical English name -> all known names (for expansion)
	fromCanonical map[string][]string
}

// NewSynonymIndex builds a synonym index from medication mappings.
func NewSynonymIndex(mappings []*domain.MedicationMapping) *SynonymIndex {
	idx := &SynonymIndex{
		toCanonical:   make(map[string]string),
		fromCanonical: make(map[string][]string),
	}

	for _, m := range mappings {
		canonical := m.EnglishName
		if canonical == "" {
			continue
		}
		canonicalLower := strings.ToLower(canonical)

		// Collect all known names for this medication
		allNames := []string{m.EnglishName, m.ArabicName}
		allNames = append(allNames, m.Synonyms...)

		// Map each name -> canonical
		for _, name := range allNames {
			if name == "" {
				continue
			}
			normalized := strings.ToLower(strings.TrimSpace(name))
			idx.toCanonical[normalized] = canonical
		}

		// Map canonical -> all names
		idx.fromCanonical[canonicalLower] = allNames
	}

	return idx
}

// GetCanonical returns the canonical English name for a medication.
// Returns empty string if not found.
func (idx *SynonymIndex) GetCanonical(name string) string {
	if idx == nil {
		return ""
	}
	normalized := strings.ToLower(strings.TrimSpace(name))
	return idx.toCanonical[normalized]
}

// GetAllSynonyms returns all known names for a medication.
// Returns slice with just the input name if not found.
func (idx *SynonymIndex) GetAllSynonyms(name string) []string {
	if idx == nil {
		return []string{name}
	}

	normalized := strings.ToLower(strings.TrimSpace(name))

	// First, find canonical name
	canonical := idx.toCanonical[normalized]
	if canonical == "" {
		return []string{name} // Unknown medication
	}

	// Get all synonyms for canonical
	allSyns := idx.fromCanonical[strings.ToLower(canonical)]
	if len(allSyns) == 0 {
		return []string{name}
	}

	return allSyns
}

// AreSynonyms returns true if two medication names refer to the same drug.
func (idx *SynonymIndex) AreSynonyms(name1, name2 string) bool {
	if idx == nil {
		return strings.EqualFold(name1, name2)
	}

	canonical1 := idx.GetCanonical(name1)
	canonical2 := idx.GetCanonical(name2)

	// If both unknown, fall back to direct comparison
	if canonical1 == "" && canonical2 == "" {
		return strings.EqualFold(name1, name2)
	}

	// If only one is known, check if the other matches it
	if canonical1 == "" {
		return strings.EqualFold(name1, canonical2)
	}
	if canonical2 == "" {
		return strings.EqualFold(canonical1, name2)
	}

	// Both known - compare canonicals
	return strings.EqualFold(canonical1, canonical2)
}

// Size returns the number of unique medications in the index.
func (idx *SynonymIndex) Size() int {
	if idx == nil {
		return 0
	}
	return len(idx.fromCanonical)
}

// TotalMappings returns the total number of name -> canonical mappings.
func (idx *SynonymIndex) TotalMappings() int {
	if idx == nil {
		return 0
	}
	return len(idx.toCanonical)
}
