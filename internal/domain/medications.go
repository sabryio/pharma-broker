package domain

import (
	"encoding/json"
	"os"
)

// LoadMedicationMappings loads the mapping from a JSON file
func LoadMedicationMappings(path string) (map[string]string, error) {
	file, err := os.Open(path)
	if err != nil {
		if os.IsNotExist(err) {
			return map[string]string{}, nil // Return empty if missing
		}
		return nil, err
	}
	defer file.Close()

	var mappings map[string]string
	if err := json.NewDecoder(file).Decode(&mappings); err != nil {
		return nil, err
	}

	return mappings, nil
}
