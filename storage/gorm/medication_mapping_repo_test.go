package gorm

import (
	"pharmabroker/domain/entity"
	"testing"
	"time"
)

// =============================================================================
// MedicationMappingRepo Tests
// =============================================================================

func TestMedicationMappingRepo_Save(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	mapping := NewTestMedicationMapping()
	err := repo.Save(ctx, mapping)
	assertNoError(t, err, "Save should succeed")

	// Verify via GetByArabicName
	found, err := repo.GetByArabicName(ctx, mapping.ArabicName)
	assertNoError(t, err, "GetByArabicName should succeed")
	assertNotNil(t, found, "Should find the mapping")
	assertEqual(t, found.EnglishName, mapping.EnglishName, "EnglishName should match")
}

func TestMedicationMappingRepo_GetByArabicName_Found(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create mapping
	mapping := NewTestMedicationMapping(func(m *entity.MedicationMapping) {
		m.ArabicName = "بانادول"
		m.EnglishName = "Panadol"
	})
	assertNoError(t, repo.Save(ctx, mapping), "Save should succeed")

	// Find by Arabic name
	found, err := repo.GetByArabicName(ctx, "بانادول")
	assertNoError(t, err, "GetByArabicName should succeed")
	assertNotNil(t, found, "Should find the mapping")
	assertEqual(t, found.EnglishName, "Panadol", "EnglishName should match")
}

func TestMedicationMappingRepo_GetByArabicName_NotFound(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	found, err := repo.GetByArabicName(ctx, "غير موجود")
	assertNoError(t, err, "GetByArabicName should not error")
	assertNil(t, found, "Should return nil for non-existent")
}

func TestMedicationMappingRepo_GetAll(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create 3 mappings
	for i := 0; i < 3; i++ {
		mapping := NewTestMedicationMapping(func(m *entity.MedicationMapping) {
			m.ArabicName = "دواء " + string(rune('أ'+i))
		})
		assertNoError(t, repo.Save(ctx, mapping), "Save should succeed")
	}

	// Get all
	all, err := repo.GetAll(ctx)
	assertNoError(t, err, "GetAll should succeed")
	assertEqual(t, len(all), 3, "Should have 3 mappings")
}

func TestMedicationMappingRepo_GetAll_Empty(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Get all on empty DB
	all, err := repo.GetAll(ctx)
	assertNoError(t, err, "GetAll should succeed on empty DB")
	assertEqual(t, len(all), 0, "Should return empty slice")
}

func TestMedicationMappingRepo_Save_Update(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Create mapping
	mapping := NewTestMedicationMapping()
	assertNoError(t, repo.Save(ctx, mapping), "First save should succeed")

	// Update synonyms
	mapping.Synonyms = []string{"Synonym1", "Synonym2", "Synonym3"}
	mapping.UpdatedAt = time.Now()
	err := repo.Save(ctx, mapping)
	assertNoError(t, err, "Update should succeed")

	// Verify update
	found, err := repo.GetByArabicName(ctx, mapping.ArabicName)
	assertNoError(t, err, "GetByArabicName should succeed")
	assertEqual(t, len(found.Synonyms), 3, "Should have 3 synonyms")
}

func TestMedicationMappingRepo_ArabicUnicode(t *testing.T) {
	db := SetupTestDB(t)
	defer db.Close()

	repo := NewMedicationMappingRepo(db.DB)
	ctx, cancel := testCtx()
	defer cancel()

	// Test complex Arabic names
	testCases := []struct {
		arabic  string
		english string
	}{
		{"أوجمنتين 1 جرام", "Augmentin 1g"},
		{"بانادول إكسترا", "Panadol Extra"},
		{"كاتافلام ٥٠ ملج", "Cataflam 50mg"},
	}

	for _, tc := range testCases {
		mapping := NewTestMedicationMapping(func(m *entity.MedicationMapping) {
			m.ArabicName = tc.arabic
			m.EnglishName = tc.english
		})
		assertNoError(t, repo.Save(ctx, mapping), "Save should succeed for: "+tc.arabic)
	}

	// Verify all were saved correctly
	for _, tc := range testCases {
		found, err := repo.GetByArabicName(ctx, tc.arabic)
		assertNoError(t, err, "GetByArabicName should succeed for: "+tc.arabic)
		assertNotNil(t, found, "Should find: "+tc.arabic)
		assertEqual(t, found.EnglishName, tc.english, "EnglishName should match for: "+tc.arabic)
	}
}
