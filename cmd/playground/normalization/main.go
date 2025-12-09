package main

import (
	"context"
	"fmt"
	"os"
	"time"

	"pharmabroker/internal/ai"
	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
	storageGorm "pharmabroker/storage/gorm"

	"github.com/rs/zerolog"
)

// Test Content with Arabic variant spellings
const content string = `*محتاج*
*أوفتريل 250*
*ديكابيبتايل*
*مريوفيرت 150*`

// Playground for testing Arabic normalization
// These variants should now match thanks to normalization:
// - أوفتريل (with hamza) should match اوفتريل (without hamza)
// - ديكابيبتايل should match ديكابيبتيل (different vowel pattern)
// - مريوفيرت should match ميريوفيرت (missing ي)

func main() {
	log := zerolog.New(os.Stdout).With().Timestamp().Logger()

	fmt.Println("=== Playground: Arabic Normalization Test ===")
	fmt.Println()

	// Test the normalizer directly
	testCases := []struct {
		input    string
		expected string
	}{
		{"أوفتريل", "اوفتريل"}, // Hamza normalization
		{"إنسولين", "انسولين"}, // Hamza below
		{"آلام", "الام"},       // Madda
		{"مستشفى", "مستشفي"},   // Alef maksura → Ya
		{"صيدلية", "صيدليه"},   // Taa marbuta → Ha
	}

	fmt.Println("Normalization Tests:")
	for _, tc := range testCases {
		result := ai.NormalizeArabic(tc.input)
		status := "✓"
		if result != tc.expected {
			status = fmt.Sprintf("✗ (got: %s)", result)
		}
		fmt.Printf("  %s: %s → %s %s\n", tc.input, result, tc.expected, status)
	}

	fmt.Println()

	// Test with real DB
	dbCfg := &storageGorm.Config{Path: "data/pharmabroker.db"}
	gormDB, err := storageGorm.NewDB(dbCfg)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to open DB")
	}
	repo := storageGorm.NewMedicationMappingRepo(gormDB)
	allMappings, _ := repo.GetAll(context.Background())

	mappings := make(map[string]string)
	for _, m := range allMappings {
		mappings[m.ArabicName] = m.EnglishName
	}

	fmt.Printf("Loaded %d mappings from DB\n\n", len(mappings))

	// Test keyword filtering with normalization
	fmt.Println("Testing FilterMappingsByKeyword with normalization:")
	fmt.Printf("Content: %s\n\n", content)

	filtered := ai.FilterMappingsByKeyword(content, mappings)
	fmt.Printf("Matched %d mappings:\n", len(filtered))
	for arabic, english := range filtered {
		fmt.Printf("  ✓ %s → %s\n", arabic, english)
	}

	// Live AI test
	fmt.Println("\n=== Live AI Test ===")

	cfg := &config.DockerModelConfig{
		BaseURL:            "http://localhost:12434/engines/llama.cpp/v1",
		Model:              "ai/qwen3-vl:latest",
		RequestTimeout:     2 * time.Minute,
		MaxRetries:         1,
		RetryBaseDelay:     time.Second,
		EmbeddingModelName: "ai/embeddinggemma",
	}

	client, err := ai.NewDockerModelClient(cfg, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create client")
	}

	client.SetMappings(allMappings)

	msg := &domain.RawMessage{
		ID:         "normalization-test",
		Content:    content,
		SenderName: "Test",
		GroupName:  "Test",
	}

	start := time.Now()
	results, err := client.ParseMessages(context.Background(), []*domain.RawMessage{msg}, ai.MapToMedicationMappings(mappings))
	duration := time.Since(start)

	if err != nil {
		log.Fatal().Err(err).Msg("AI parsing failed")
	}

	fmt.Printf("\nParsed in %v\n", duration)
	fmt.Println("Results:")
	for _, res := range results {
		for i, item := range res.Items {
			// Check if correctly mapped
			status := "?"
			switch item.Medication {
			case "Ovitrelle", "Ovitrelle 250":
				status = "✓ Ovitrelle"
			case "Decapeptyl":
				status = "✓ Decapeptyl"
			case "Mireofert", "Mireofert 150":
				status = "✓ Mireofert"
			}
			fmt.Printf("  [%d] %s (Raw: %s) %s\n", i, item.Medication, item.MedicationRaw, status)
		}
	}
}
