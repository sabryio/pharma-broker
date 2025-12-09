package domain

import (
	"encoding/json"
	"os"
	"pharmabroker/domain/entity"
)

// MedicationEntry represents a medication in the rich JSON format
type MedicationEntry struct {
	English  string   `json:"english"`
	Synonyms []string `json:"synonyms,omitempty"`
}

// LoadRichMedicationMappings loads the new format with synonyms
// Format: { "arabic_name": { "english": "...", "synonyms": ["...", "..."] } }
func LoadRichMedicationMappings(path string) ([]*entity.MedicationMapping, error) {
	file, err := os.Open(path)
	if err != nil {
		if os.IsNotExist(err) {
			return []*entity.MedicationMapping{}, nil
		}
		return nil, err
	}
	defer file.Close()

	// Try rich format first
	var richMappings map[string]MedicationEntry
	decoder := json.NewDecoder(file)
	if err := decoder.Decode(&richMappings); err == nil {
		// Successfully parsed as rich format
		var result []*entity.MedicationMapping
		for arabic, entry := range richMappings {
			result = append(result, &entity.MedicationMapping{
				ArabicName:  arabic,
				EnglishName: entry.English,
				Synonyms:    entry.Synonyms,
			})
		}
		return result, nil
	}

	// Fallback: try legacy flat format
	file.Seek(0, 0) // Reset file position
	var flatMappings map[string]string
	if err := json.NewDecoder(file).Decode(&flatMappings); err != nil {
		return nil, err
	}

	var result []*entity.MedicationMapping
	for arabic, english := range flatMappings {
		result = append(result, &entity.MedicationMapping{
			ArabicName:  arabic,
			EnglishName: english,
		})
	}
	return result, nil
}
