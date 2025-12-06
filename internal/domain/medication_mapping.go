package domain

import (
	"time"
)

// MedicationMapping represents a mapping between Arabic and English medication names
type MedicationMapping struct {
	ID          string    `json:"id"`
	ArabicName  string    `json:"arabic_name"` // Canonical Arabic name
	EnglishName string    `json:"english_name"`
	Synonyms    []string  `json:"synonyms"` // Alternative spellings/names
	CreatedAt   time.Time `json:"created_at"`
	UpdatedAt   time.Time `json:"updated_at"`
}
