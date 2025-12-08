package main

import (
	"context"
	"fmt"
	"os"
	"time"

	"pharmabroker/internal/ai"
	"pharmabroker/internal/config"
	"pharmabroker/internal/storage"

	"github.com/rs/zerolog"
)

// Tool to refresh/regenerate embeddings for all medication mappings
// Usage: go run cmd/tools/refresh_embeddings/main.go

func main() {
	log := zerolog.New(os.Stdout).With().Timestamp().Logger()

	fmt.Println("=== Medication Embedding Refresh Tool ===")
	fmt.Println()

	// Load configuration
	dbPath := os.Getenv("DB_PATH")
	if dbPath == "" {
		dbPath = "data/pharmabroker.db"
	}

	dbCfg := &config.DatabaseConfig{Path: dbPath}
	gormDB, err := storage.NewGormDB(dbCfg)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to open database")
	}

	repo := storage.NewGormMedicationMappingRepo(gormDB)

	// Fetch all mappings
	allMappings, err := repo.GetAll(context.Background())
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to fetch mappings")
	}

	fmt.Printf("Found %d medication mappings\n\n", len(allMappings))

	// Setup AI client for embeddings
	aiCfg := &config.DockerModelConfig{
		BaseURL:            os.Getenv("LLM_URL"),
		Model:              "ai/qwen3-vl:latest",
		RequestTimeout:     2 * time.Minute,
		MaxRetries:         3,
		RetryBaseDelay:     time.Second,
		EmbeddingModelName: "ai/embeddinggemma",
	}

	if aiCfg.BaseURL == "" {
		aiCfg.BaseURL = "http://localhost:12434/engines/llama.cpp/v1"
	}

	client, err := ai.NewDockerModelClient(aiCfg, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create AI client")
	}

	// Count existing embeddings
	needsEmbedding := 0
	hasEmbedding := 0
	for _, m := range allMappings {
		if len(m.Embedding) == 0 {
			needsEmbedding++
		} else {
			hasEmbedding++
		}
	}

	fmt.Printf("Existing embeddings: %d\n", hasEmbedding)
	fmt.Printf("Need embedding: %d\n\n", needsEmbedding)

	if needsEmbedding == 0 {
		fmt.Println("✓ All mappings already have embeddings!")
		return
	}

	// Generate embeddings in batches
	batchSize := 10
	arabicNames := make([]string, 0, needsEmbedding)
	indexMap := make(map[int]int)

	for i, m := range allMappings {
		if len(m.Embedding) == 0 {
			indexMap[len(arabicNames)] = i
			arabicNames = append(arabicNames, m.ArabicName)
		}
	}

	fmt.Printf("Generating embeddings for %d medications...\n", len(arabicNames))
	start := time.Now()

	for i := 0; i < len(arabicNames); i += batchSize {
		end := min(i+batchSize, len(arabicNames))

		batch := arabicNames[i:end]
		fmt.Printf("  Batch %d-%d/%d...\n", i+1, end, len(arabicNames))

		embeddings, err := client.EmbedBatch(context.Background(), batch)
		if err != nil {
			log.Error().Err(err).Int("batch_start", i).Msg("Failed to generate embeddings for batch")
			continue
		}

		// Store embeddings
		for j, emb := range embeddings {
			idx := indexMap[i+j]
			allMappings[idx].Embedding = emb
			if err := repo.Save(context.Background(), allMappings[idx]); err != nil {
				log.Error().Err(err).Str("name", allMappings[idx].ArabicName).Msg("Failed to save embedding")
			}
		}
	}

	duration := time.Since(start)
	fmt.Printf("\n✓ Generated %d embeddings in %v\n", len(arabicNames), duration)
}
