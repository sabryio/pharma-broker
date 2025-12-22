package similarity

// Comparator defines a pluggable similarity algorithm.
type Comparator interface {
	Similarity(a, b []float32) float32
}
