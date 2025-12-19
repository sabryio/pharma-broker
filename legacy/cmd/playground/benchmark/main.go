package main

import (
	"context"
	"fmt"
	"os"
	"strings"
	"time"

	aiDocker "pharmabroker/ai/docker"
	"pharmabroker/domain/entity"
	"pharmabroker/pkg/config"
	"pharmabroker/pkg/matcher/filtering"
	storageGorm "pharmabroker/storage/gorm"

	"github.com/rs/zerolog"
)

// Benchmark tool for measuring hybrid RAG filtering performance impact
// Usage: go run cmd/playground/benchmark/main.go

func main() {
	log := zerolog.New(os.Stdout).With().Timestamp().Logger()

	fmt.Println("=== Hybrid RAG Filtering Benchmark ===")
	fmt.Println()

	// Load config
	cfg := config.Load()
	cfg.AI.Provider = "docker"

	// Override base URL for local development if not set
	if cfg.DockerModel.BaseURL == "" || strings.Contains(cfg.DockerModel.BaseURL, "docker.internal") {
		aiURL := os.Getenv("LLM_URL")
		if aiURL == "" {
			aiURL = "http://localhost:12434/engines/llama.cpp/v1"
		}
		cfg.DockerModel.BaseURL = aiURL
	}

	ctx := context.Background()

	// Setup DB and mappings
	gormDB, err := storageGorm.NewDB(&storageGorm.Config{DSN: "postgres://postgres:password@localhost:5432/pharmabroker?sslmode=disable"})
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to open DB")
	}
	repo := storageGorm.NewMedicationMappingRepo(gormDB)
	allMappings, _ := repo.GetAll(ctx)

	// Create full mappings map
	fullMappings := make(map[string]string)
	for _, m := range allMappings {
		fullMappings[m.ArabicName] = m.EnglishName
	}

	fmt.Printf("Total mappings in database: %d\n\n", len(fullMappings))

	// Create AI client
	client, err := aiDocker.NewClient(&cfg.DockerModel, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create AI client")
	}

	// Test messages
	testMessages := []*entity.RawMessage{
		{ID: "bench-1", Content: "*عندي*\n*زولادكس 3.6*\n*سكسندا*\n*اوزمبك*"},
		{ID: "bench-2", Content: "*محتاج*\n*ديكابيبتيل*\n*فوستيمون*"},
		{ID: "bench-3", Content: "*متوفر*\n*مريوفيرت*\n*سيتروتايد*\n*اوفتريل*"},
	}

	fmt.Println("--- Test 1: WITHOUT Hybrid Filtering ---")
	runBenchmark(ctx, client, testMessages, fullMappings, false, allMappings)

	fmt.Println("\n--- Test 2: WITH Hybrid Filtering ---")
	runBenchmark(ctx, client, testMessages, fullMappings, true, allMappings)
}

func runBenchmark(ctx context.Context, client *aiDocker.Client, messages []*entity.RawMessage, mappings map[string]string, useHybrid bool, allMappings []*entity.MedicationMapping) {
	iterations := 3

	if useHybrid {
		client.SetMappings(allMappings)
	} else {
		client.SetMappings(nil) // Clear mappings to disable hybrid
	}

	var totalDuration time.Duration
	var totalItems int

	for i := range iterations {
		start := time.Now()
		results, err := client.ParseMessages(ctx, messages, filtering.MapToMappingsEntity(mappings))
		duration := time.Since(start)

		if err != nil {
			fmt.Printf("  Iteration %d: ERROR - %v\n", i+1, err)
			continue
		}

		itemCount := 0
		for _, r := range results {
			itemCount += len(r.Items)
		}

		totalDuration += duration
		totalItems += itemCount
		fmt.Printf("  Iteration %d: %v (%d items parsed)\n", i+1, duration, itemCount)
	}

	avgDuration := totalDuration / time.Duration(iterations)
	avgItems := float64(totalItems) / float64(iterations)

	fmt.Printf("\n  Average: %v (%.1f items per run)\n", avgDuration, avgItems)

	// Measure token count
	promptTokens := estimatePromptTokens(messages, mappings, useHybrid, allMappings)
	fmt.Printf("  Estimated prompt tokens: ~%d\n", promptTokens)
}

func estimatePromptTokens(messages []*entity.RawMessage, mappings map[string]string, useHybrid bool, _ []*entity.MedicationMapping) int {
	// Rough estimate: 4 chars per token
	var textLen int

	for _, m := range messages {
		textLen += len(m.Content)
	}

	if useHybrid {
		// Filtered mappings would be much smaller
		// Estimate: ~10 mappings per message on average
		filteredCount := min(len(mappings), len(messages)*10)
		textLen += filteredCount * 40 // Average Arabic+English per mapping
	} else {
		// Full mappings
		textLen += len(mappings) * 40
	}

	return textLen / 4
}
