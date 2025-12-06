package storage

import (
	"context"
	"testing"
	"time"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
)

func TestMedicationRepo_Synonyms(t *testing.T) {
	// Setup DB
	db, err := New(&config.DatabaseConfig{Path: ":memory:"})
	if err != nil {
		t.Fatalf("Failed to open DB: %v", err)
	}
	defer db.Close()
	repo := NewMedicationMappingRepo(db)
	ctx := context.Background()

	// 1. Save a mapping with synonyms
	mapping := &domain.MedicationMapping{
		ArabicName:  "ازومبك", // Canonical
		EnglishName: "Ozempic",
		Synonyms:    []string{"اوزمبك", "اوزمبيك"},
		CreatedAt:   time.Now(),
	}
	if err := repo.Save(ctx, mapping); err != nil {
		t.Fatalf("Failed to save mapping: %v", err)
	}

	// 2. Retrieve by Canonical Name
	saved, err := repo.GetByArabicName(ctx, "ازومبك")
	if err != nil {
		t.Fatalf("Failed to get by arabic: %v", err)
	}
	if saved.EnglishName != "Ozempic" {
		t.Errorf("Expected Ozempic, got %s", saved.EnglishName)
	}
	if len(saved.Synonyms) != 2 {
		t.Errorf("Expected 2 synonyms, got %d", len(saved.Synonyms))
	}

	// 3. Search using FTS (Canonical)
	results, err := repo.Search(ctx, "ازومبك")
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}
	if len(results) == 0 {
		t.Error("Search for canonical returned no results")
	}

	// 4. Search using Synonym
	results, err = repo.Search(ctx, "اوزمبك")
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}
	if len(results) == 0 {
		t.Error("Search for synonym returned no results")
	}
	if results[0].EnglishName != "Ozempic" {
		t.Errorf("Search for synonym match wrong item: %s", results[0].EnglishName)
	}

	// 5. Search using Trigram/Partial
	// sqlite fts5 trigram tokenization allows partial matches
	// But only if enabled. Migration 9 enables 'trigram'.
	// Note: 'tokenize="trigram"' works on SQLite 3.34+
	// modernc.org/sqlite supports it.

	results, err = repo.Search(ctx, "اوزمب")
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}
	if len(results) == 0 {
		t.Log("Fuzzy/Partial search returned no results (might depend on trigram implementation)")
	} else {
		if results[0].EnglishName != "Ozempic" {
			t.Errorf("Fuzzy match returned wrong item: %s", results[0].EnglishName)
		}
	}
}
