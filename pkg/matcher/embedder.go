package matcher

import "context"

// Embedder generates vectors for text inputs.
type Embedder interface {
	Embed(ctx context.Context, text string) ([]float32, error)
}
