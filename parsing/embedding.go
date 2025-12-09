package parsing

import (
	"context"
	"strings"
	"sync"

	"pharmabroker/domain/repository"
	synonymsPkg "pharmabroker/pkg/synonyms"

	"github.com/rs/zerolog"
)

// EmbeddingCache manages in-memory embeddings and synonym index
type EmbeddingCache struct {
	mu           sync.RWMutex
	embeddings   map[string][]float32
	synonymIndex *synonymsPkg.SynonymIndex
	repo         repository.MedicationMappingRepository
	log          zerolog.Logger
}

// NewEmbeddingCache creates a new embedding cache
func NewEmbeddingCache(repo repository.MedicationMappingRepository, log zerolog.Logger) *EmbeddingCache {
	return &EmbeddingCache{
		embeddings: make(map[string][]float32),
		repo:       repo,
		log:        log,
	}
}

// Refresh loads embeddings and synonym index from the repository
func (ec *EmbeddingCache) Refresh(ctx context.Context) error {
	mappings, err := ec.repo.GetAll(ctx)
	if err != nil {
		return err
	}

	newEmbeddings := make(map[string][]float32)
	count := 0

	for _, m := range mappings {
		if len(m.Embedding) == 0 {
			continue // Skip if no embedding
		}

		// Map all known names to this embedding
		if m.ArabicName != "" {
			newEmbeddings[strings.ToLower(m.ArabicName)] = m.Embedding
		}
		if m.EnglishName != "" {
			newEmbeddings[strings.ToLower(m.EnglishName)] = m.Embedding
		}
		for _, syn := range m.Synonyms {
			if syn != "" {
				newEmbeddings[strings.ToLower(syn)] = m.Embedding
			}
		}
		count++
	}

	ec.mu.Lock()
	ec.embeddings = newEmbeddings
	ec.synonymIndex = synonymsPkg.NewSynonymIndex(mappings)
	ec.mu.Unlock()

	ec.log.Info().
		Int("embeddings", count).
		Int("embedding_keys", len(newEmbeddings)).
		Int("synonym_medications", ec.synonymIndex.Size()).
		Int("synonym_mappings", ec.synonymIndex.TotalMappings()).
		Msg("Refreshed in-memory embeddings and synonym index")
	return nil
}

// GetEmbedding returns the embedding vector for a given term
func (ec *EmbeddingCache) GetEmbedding(term string) ([]float32, bool) {
	ec.mu.RLock()
	defer ec.mu.RUnlock()
	vec, ok := ec.embeddings[strings.ToLower(term)]
	return vec, ok
}

// AreSynonyms checks if two terms are synonyms
func (ec *EmbeddingCache) AreSynonyms(term1, term2 string) bool {
	ec.mu.RLock()
	defer ec.mu.RUnlock()
	if ec.synonymIndex == nil {
		return false
	}
	return ec.synonymIndex.AreSynonyms(term1, term2)
}
