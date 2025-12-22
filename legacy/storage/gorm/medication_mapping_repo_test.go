package gorm

import (
	"pharmabroker/domain/entity"
	"testing"
)

func TestMedicationMappingRepo_Save_Embedding(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Helper to create vector of size 768
	makeVec := func(val float32) []float32 {
		v := make([]float32, 768)
		for i := range v {
			v[i] = val
		}
		return v
	}

	// 1. Create mapping with embedding
	mapping := &entity.MedicationMapping{
		ArabicName:  "ستربسلس",
		EnglishName: "Strepsils",
		Synonyms:    []string{"Strepsils Honey"},
		Embedding:   makeVec(0.1),
	}

	err := repo.Save(ctx, mapping)
	assertNoError(t, err, "Save should succeed")

	// 2. Retrieve and verify embedding
	saved, err := repo.GetByArabicName(ctx, mapping.ArabicName)
	assertNoError(t, err, "GetByArabicName should succeed")

	if len(saved.Embedding) != 768 {
		t.Fatalf("Expected embedding length 768, got %d", len(saved.Embedding))
	}

	if saved.Embedding[0] != 0.1 {
		t.Errorf("Embedding values mismatch. Got %v...", saved.Embedding[:5])
	}

	// 3. Update embedding
	mapping.Embedding = makeVec(0.9)
	err = repo.Save(ctx, mapping)
	assertNoError(t, err, "Update should succeed")

	updated, err := repo.GetByArabicName(ctx, mapping.ArabicName)
	assertNoError(t, err, "GetByArabicName should succeed after update")

	if updated.Embedding[0] != 0.9 {
		t.Errorf("Update failed. Got %v...", updated.Embedding[:5])
	}
}

func TestMedicationMappingRepo_FindSimilar(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Helper to create vector with specific values in first few dimensions
	makeVec := func(vals ...float32) []float32 {
		v := make([]float32, 768)
		copy(v, vals)
		return v
	}

	// Add items with different embeddings
	// m1: [1, 0, 0...] -> Target
	// m2: [0, 1, 0...] -> Orthogonal (different)
	// m3: [0.9, 0.1, 0...] -> Close to m1
	m1 := &entity.MedicationMapping{ArabicName: "A", EnglishName: "A", Embedding: makeVec(1.0, 0.0, 0.0)}
	m2 := &entity.MedicationMapping{ArabicName: "B", EnglishName: "B", Embedding: makeVec(0.0, 1.0, 0.0)}
	m3 := &entity.MedicationMapping{ArabicName: "C", EnglishName: "C", Embedding: makeVec(0.9, 0.1, 0.0)}

	assertNoError(t, repo.Save(ctx, m1), "Save m1")
	assertNoError(t, repo.Save(ctx, m2), "Save m2")
	assertNoError(t, repo.Save(ctx, m3), "Save m3")

	count, _ := repo.Count(ctx)
	if count != 3 {
		t.Logf("Expected 3 items, got %d", count)
	}

	// Search similar to m1 ([1, 0, 0])
	// Expected Order:
	// 1. A (Distance 0)
	// 2. C (Distance small)
	// 3. B (Distance large/1.0)
	queryVec := makeVec(1.0, 0.0, 0.0)
	results, err := repo.FindSimilar(ctx, queryVec, 3)
	assertNoError(t, err, "FindSimilar should succeed")

	if len(results) < 3 {
		t.Fatalf("Expected 3 results, got %d", len(results))
	}

	if results[0].ArabicName != "A" {
		t.Errorf("Expected first result to be A, got %s", results[0].ArabicName)
	}
	if results[1].ArabicName != "C" {
		t.Errorf("Expected second result to be C, got %s", results[1].ArabicName)
	}
}

func TestMedicationMappingRepo_GetAll(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	assertNoError(t, repo.Save(ctx, &entity.MedicationMapping{ID: "1", ArabicName: "A", EnglishName: "A"}), "Save 1")
	assertNoError(t, repo.Save(ctx, &entity.MedicationMapping{ID: "2", ArabicName: "B", EnglishName: "B"}), "Save 2")

	all, err := repo.GetAll(ctx)
	assertNoError(t, err, "GetAll should succeed")
	assertEqual(t, len(all), 2, "Should return 2 items")
}

func TestMedicationMappingRepo_Search(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Add items for fuzzy search
	// Panadol Extra
	m1 := &entity.MedicationMapping{
		ArabicName:  "بانادول اكسترا",
		EnglishName: "Panadol Extra",
	}
	// Augmentin
	m2 := &entity.MedicationMapping{
		ArabicName:  "أوجمنتين",
		EnglishName: "Augmentin",
	}

	assertNoError(t, repo.Save(ctx, m1), "Save m1")
	assertNoError(t, repo.Save(ctx, m2), "Save m2")

	// Lower threshold for test to ensure fuzzy match works with short query vs long name
	db.Conn.Exec("SELECT set_limit(0.1);")

	// Search with typo "بنادول" (missing alef)
	results, err := repo.Search(ctx, "بنادول")
	assertNoError(t, err, "Search should succeed")

	if len(results) == 0 {
		t.Fatal("Expected results for fuzzy search")
	}
	if results[0].EnglishName != "Panadol Extra" {
		t.Errorf("Expected 'Panadol Extra', got '%s'", results[0].EnglishName)
	}

	// Search by English check
	resultsEng, err := repo.Search(ctx, "Augmuntin") // typo
	assertNoError(t, err, "Search English should succeed")
	if len(resultsEng) == 0 {
		t.Fatal("Expected results for English fuzzy search")
	}
	if resultsEng[0].ArabicName != "أوجمنتين" {
		t.Errorf("Expected 'أوجمنتين', got '%s'", resultsEng[0].ArabicName)
	}
}

func TestMedicationMappingRepo_Save_Synonyms(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	mapping := &entity.MedicationMapping{
		ArabicName:  "باي الكوفان",
		EnglishName: "Bi-Alcofan",
		Synonyms:    []string{"BiAlcofan", "Bi Alcofan"},
	}

	err := repo.Save(ctx, mapping)
	assertNoError(t, err, "Save should succeed")

	saved, err := repo.GetByArabicName(ctx, mapping.ArabicName)
	assertNoError(t, err, "Get should succeed")

	assertEqual(t, len(saved.Synonyms), 2, "Should have 2 synonyms")
	if saved.Synonyms[0] != "BiAlcofan" {
		t.Errorf("Synonym mismatch")
	}
}
