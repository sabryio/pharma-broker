package parsing

import (
	"context"
	"testing"

	"pharmabroker/internal/domain"

	"github.com/rs/zerolog"
)

func TestNewEmbeddingCache(t *testing.T) {
	repo := &MockMedicationRepo{}
	logger := zerolog.Nop()

	cache := NewEmbeddingCache(repo, logger)

	if cache == nil {
		t.Fatal("NewEmbeddingCache returned nil")
	}
	if cache.embeddings == nil {
		t.Error("embeddings map not initialized")
	}
}

func TestEmbeddingCache_Refresh(t *testing.T) {
	tests := []struct {
		name         string
		mappings     []*domain.MedicationMapping
		expectedKeys int
	}{
		{
			name: "with embeddings",
			mappings: []*domain.MedicationMapping{
				{
					ArabicName:  "باراسيتامول",
					EnglishName: "Paracetamol",
					Embedding:   []float32{0.1, 0.2, 0.3},
					Synonyms:    []string{"Acetaminophen"},
				},
			},
			expectedKeys: 3, // Arabic, English, and synonym
		},
		{
			name: "without embeddings",
			mappings: []*domain.MedicationMapping{
				{
					ArabicName:  "أسبرين",
					EnglishName: "Aspirin",
					Embedding:   nil, // No embedding
				},
			},
			expectedKeys: 0,
		},
		{
			name:         "empty mappings",
			mappings:     []*domain.MedicationMapping{},
			expectedKeys: 0,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			repo := &MockMedicationRepo{
				OnGetAll: func(ctx context.Context) ([]*domain.MedicationMapping, error) {
					return tt.mappings, nil
				},
			}
			logger := zerolog.Nop()
			cache := NewEmbeddingCache(repo, logger)

			err := cache.Refresh(context.Background())
			if err != nil {
				t.Fatalf("Refresh failed: %v", err)
			}

			cache.mu.RLock()
			keyCount := len(cache.embeddings)
			cache.mu.RUnlock()

			if keyCount != tt.expectedKeys {
				t.Errorf("Expected %d keys, got %d", tt.expectedKeys, keyCount)
			}
		})
	}
}

func TestEmbeddingCache_GetEmbedding(t *testing.T) {
	repo := &MockMedicationRepo{
		OnGetAll: func(ctx context.Context) ([]*domain.MedicationMapping, error) {
			return []*domain.MedicationMapping{
				{
					ArabicName:  "test",
					EnglishName: "Test",
					Embedding:   []float32{0.1, 0.2, 0.3},
				},
			}, nil
		},
	}
	logger := zerolog.Nop()
	cache := NewEmbeddingCache(repo, logger)
	_ = cache.Refresh(context.Background())

	tests := []struct {
		name    string
		term    string
		wantOk  bool
		wantLen int
	}{
		{"exact match lowercase", "test", true, 3},
		{"exact match uppercase", "Test", true, 3},
		{"exact match mixed", "TEST", true, 3},
		{"not found", "nonexistent", false, 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			vec, ok := cache.GetEmbedding(tt.term)
			if ok != tt.wantOk {
				t.Errorf("GetEmbedding(%q) ok = %v, want %v", tt.term, ok, tt.wantOk)
			}
			if len(vec) != tt.wantLen {
				t.Errorf("GetEmbedding(%q) len = %d, want %d", tt.term, len(vec), tt.wantLen)
			}
		})
	}
}

func TestEmbeddingCache_AreSynonyms(t *testing.T) {
	repo := &MockMedicationRepo{
		OnGetAll: func(ctx context.Context) ([]*domain.MedicationMapping, error) {
			return []*domain.MedicationMapping{
				{
					ArabicName:  "باراسيتامول",
					EnglishName: "Paracetamol",
					Embedding:   []float32{0.1, 0.2, 0.3},
					Synonyms:    []string{"Acetaminophen", "Tylenol"},
				},
			}, nil
		},
	}
	logger := zerolog.Nop()
	cache := NewEmbeddingCache(repo, logger)
	_ = cache.Refresh(context.Background())

	tests := []struct {
		name  string
		term1 string
		term2 string
		want  bool
	}{
		{"same term", "Paracetamol", "Paracetamol", true},
		{"english and synonym", "Paracetamol", "Acetaminophen", true},
		{"synonym and synonym", "Acetaminophen", "Tylenol", true},
		{"not synonyms", "Paracetamol", "Aspirin", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := cache.AreSynonyms(tt.term1, tt.term2)
			if result != tt.want {
				t.Errorf("AreSynonyms(%q, %q) = %v, want %v", tt.term1, tt.term2, result, tt.want)
			}
		})
	}
}

func TestEmbeddingCache_AreSynonyms_NilIndex(t *testing.T) {
	repo := &MockMedicationRepo{}
	logger := zerolog.Nop()
	cache := NewEmbeddingCache(repo, logger)
	// Don't call Refresh, so synonymIndex is nil

	result := cache.AreSynonyms("test1", "test2")
	if result != false {
		t.Error("AreSynonyms should return false when synonymIndex is nil")
	}
}

// Benchmark tests
func BenchmarkEmbeddingCache_GetEmbedding(b *testing.B) {
	repo := &MockMedicationRepo{
		OnGetAll: func(ctx context.Context) ([]*domain.MedicationMapping, error) {
			return []*domain.MedicationMapping{
				{
					ArabicName:  "test",
					EnglishName: "Test",
					Embedding:   []float32{0.1, 0.2, 0.3},
				},
			}, nil
		},
	}
	logger := zerolog.Nop()
	cache := NewEmbeddingCache(repo, logger)
	_ = cache.Refresh(context.Background())

	for b.Loop() {
		cache.GetEmbedding("test")
	}
}
