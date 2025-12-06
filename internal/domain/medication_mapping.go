package domain

import (
	"time"
)

// MedicationMapping represents a mapping between Arabic and English medication names
type MedicationMapping struct {
	ID          string    `json:"id"`
	ArabicName  string    `json:"arabic_name"`
	EnglishName string    `json:"english_name"`
	CreatedAt   time.Time `json:"created_at"`
	UpdatedAt   time.Time `json:"updated_at"`
}
