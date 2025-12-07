package ai

import (
	"context"
	"math"
	"sort"
	"strings"

	"pharmabroker/internal/domain"
)

// Embedder interface for generating text embeddings
type Embedder interface {
	Embed(ctx context.Context, text string) ([]float32, error)
}

// FilterMappingsByKeyword returns only mappings where Arabic key appears in content
func FilterMappingsByKeyword(content string, mappings map[string]string) map[string]string {
	result := make(map[string]string)
	contentLower := strings.ToLower(content)

	for arabic, english := range mappings {
		if strings.Contains(contentLower, strings.ToLower(arabic)) {
			result[arabic] = english
		}
	}

	return result
}

// FilterMappingsBySimilarity returns top-K semantically similar mappings
func FilterMappingsBySimilarity(ctx context.Context, content string, allMappings []*domain.MedicationMapping, embedder Embedder, topK int) (map[string]string, error) {
	if len(allMappings) == 0 || topK <= 0 {
		return make(map[string]string), nil
	}

	// Embed the message content
	contentEmbedding, err := embedder.Embed(ctx, content)
	if err != nil {
		return nil, err
	}

	// Score all mappings by similarity
	type scoredMapping struct {
		mapping *domain.MedicationMapping
		score   float32
	}
	var scored []scoredMapping

	for _, m := range allMappings {
		if len(m.Embedding) == 0 {
			continue
		}
		score := cosineSimilarity(contentEmbedding, m.Embedding)
		scored = append(scored, scoredMapping{m, score})
	}

	// Sort by score descending
	sort.Slice(scored, func(i, j int) bool {
		return scored[i].score > scored[j].score
	})

	// Take top-K
	result := make(map[string]string)
	for i := 0; i < topK && i < len(scored); i++ {
		m := scored[i].mapping
		result[m.ArabicName] = m.EnglishName
	}

	return result, nil
}

// FilterMappingsHybrid combines keyword matching and vector similarity
// Always runs both and returns the union (deduped)
func FilterMappingsHybrid(ctx context.Context, content string, allMappings []*domain.MedicationMapping, embedder Embedder, vectorTopK int) map[string]string {
	// Build map for keyword filtering
	fullMap := make(map[string]string)
	for _, m := range allMappings {
		fullMap[m.ArabicName] = m.EnglishName
		// Include synonyms
		for _, syn := range m.Synonyms {
			fullMap[syn] = m.EnglishName
		}
	}

	// Step 1: Keyword filtering (always)
	result := FilterMappingsByKeyword(content, fullMap)

	// Step 2: Vector similarity (always add top-K)
	vectorMatches, err := FilterMappingsBySimilarity(ctx, content, allMappings, embedder, vectorTopK)
	if err == nil {
		// Merge vector matches into result (deduped by map key)
		for arabic, english := range vectorMatches {
			if _, exists := result[arabic]; !exists {
				result[arabic] = english
			}
		}
	}

	return result
}

// cosineSimilarity calculates cosine similarity between two vectors
func cosineSimilarity(a, b []float32) float32 {
	if len(a) != len(b) || len(a) == 0 {
		return 0
	}

	var dot, normA, normB float32
	for i := range a {
		dot += a[i] * b[i]
		normA += a[i] * a[i]
		normB += b[i] * b[i]
	}

	if normA == 0 || normB == 0 {
		return 0
	}

	return dot / (float32(math.Sqrt(float64(normA))) * float32(math.Sqrt(float64(normB))))
}
