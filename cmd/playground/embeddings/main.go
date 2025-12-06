package main

import (
	"context"
	"fmt"
	"os"

	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"

	"pharmabroker/internal/ai"
	"pharmabroker/internal/config"
)

func main() {
	// Setup logging
	log.Logger = log.Output(zerolog.ConsoleWriter{Out: os.Stderr})

	ctx := context.Background()

	// Load system config
	cfg := config.Load()

	// Force Docker provider for this playground
	cfg.AI.Provider = "docker"

	// Create AI Provider using factory
	// Defaults for EmbeddingBaseURL etc. are now handled in config.Load()
	client, err := ai.NewAIProvider(ctx, cfg, log.Logger)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create AI provider")
	}

	fmt.Printf("Connecting to %s using model '%s'...\n", cfg.DockerModel.BaseURL, cfg.DockerModel.EmbeddingModelName)

	// Test words
	words := []string{"Panadol", "Paracetamol", "Ibuprofen", "Car"}

	fmt.Println("\n--- Generating Embeddings ---")

	vectors := make(map[string][]float32)

	for _, word := range words {
		fmt.Printf("Embedding '%s'...", word)
		vec, err := client.Embed(ctx, word)
		if err != nil {
			fmt.Printf(" Error: %v\n", err)
			continue
		}
		fmt.Printf(" Done. Dimensions: %d\n", len(vec))
		vectors[word] = vec
	}

	fmt.Println("\n--- Cosine Similarity ---")

	pairs := [][2]string{
		{"Panadol", "Paracetamol"},
		{"Panadol", "Ibuprofen"},
		{"Panadol", "Car"},
	}

	for _, pair := range pairs {
		v1 := vectors[pair[0]]
		v2 := vectors[pair[1]]

		if v1 == nil || v2 == nil {
			continue
		}

		score := ai.CosineSimilarity(v1, v2)
		fmt.Printf("Similarity '%s' <-> '%s': %.4f\n", pair[0], pair[1], score)
	}
}
