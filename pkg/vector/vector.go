package vector

import (
	"errors"
	"math"
)

// VectorComparator defines behavior for vector similarity algorithms.
type VectorComparator interface {
	Similarity(a, b []float32) (float64, error)
}

// CosineComparator implements vector cosine similarity.
type CosineComparator struct{}

// Similarity computes cosine similarity between two vectors.
func (CosineComparator) Similarity(a, b []float32) (float64, error) {
	if len(a) != len(b) {
		return 0, errors.New("vectors must have equal length")
	}
	if len(a) == 0 {
		return 0, errors.New("vectors must not be empty")
	}

	var dot, normA, normB float64
	for i := range a {
		ai := float64(a[i])
		bi := float64(b[i])
		dot += ai * bi
		normA += ai * ai
		normB += bi * bi
	}

	if normA == 0 || normB == 0 {
		return 0, nil
	}

	return dot / (math.Sqrt(normA) * math.Sqrt(normB)), nil
}
