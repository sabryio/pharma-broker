//go:build integration
// +build integration

package ai

import (
	"context"
	"os"
	"strings"
	"testing"
	"time"

	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage"

	"github.com/rs/zerolog"
)

// Integration tests for the AI parsing pipeline
// Run with: go test -tags=integration ./internal/ai/...

func setupIntegrationClient(t *testing.T) (*DockerModelClient, map[string]string, []*domain.MedicationMapping) {
	t.Helper()

	// Check if AI service is available
	aiURL := os.Getenv("LLM_URL")
	if aiURL == "" {
		aiURL = "http://localhost:12434/engines/llama.cpp/v1"
	}

	cfg := &config.DockerModelConfig{
		BaseURL:            aiURL,
		Model:              "ai/qwen3-vl:latest",
		EmbeddingBaseURL:   aiURL,
		EmbeddingModelName: "ai/embeddinggemma",
		MaxTokens:          2048,
		Temperature:        0.1,
		MaxRetries:         3,
		RetryBaseDelay:     time.Second,
	}

	log := zerolog.New(os.Stdout).With().Timestamp().Logger()
	client, err := NewDockerModelClient(cfg, log)
	if err != nil {
		t.Skipf("Skipping integration test - AI client unavailable: %v", err)
	}

	// Load real mappings from DB
	dbPath := os.Getenv("DB_PATH")
	if dbPath == "" {
		dbPath = "../../data/pharmabroker.db" // Relative from internal/ai
	}

	dbCfg := &config.DatabaseConfig{Path: dbPath}
	gormDB, err := storage.NewGormDB(dbCfg)
	if err != nil {
		t.Skipf("Skipping integration test - DB unavailable: %v", err)
	}

	repo := storage.NewGormMedicationMappingRepo(gormDB)
	allMappings, err := repo.GetAll(context.Background())
	if err != nil {
		t.Skipf("Skipping integration test - failed to load mappings: %v", err)
	}

	mappings := make(map[string]string)
	for _, m := range allMappings {
		mappings[m.ArabicName] = m.EnglishName
	}

	// Enable hybrid filtering
	client.SetMappings(allMappings)

	return client, mappings, allMappings
}

func TestIntegration_ParseKnownMedications(t *testing.T) {
	client, mappings, _ := setupIntegrationClient(t)

	msg := &domain.RawMessage{
		ID:         "int-test-1",
		Content:    "*عندي*\n*زولادكس 3.6*\n*اوزمبك*",
		SenderName: "Test",
		GroupName:  "Integration Test",
	}

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	results, err := client.ParseMessages(ctx, []*domain.RawMessage{msg}, mappings)
	if err != nil {
		t.Fatalf("ParseMessages failed: %v", err)
	}

	if len(results) != 1 {
		t.Fatalf("Expected 1 result, got %d", len(results))
	}

	result := results[0]
	if len(result.Items) < 2 {
		t.Errorf("Expected at least 2 items, got %d", len(result.Items))
	}

	// Check that Zoladex and Ozempic are correctly mapped
	foundZoladex := false
	foundOzempic := false

	for _, item := range result.Items {
		if strings.Contains(item.Medication, "Zoladex") {
			foundZoladex = true
			if item.MatchConfidence != "EXACT" {
				t.Errorf("Zoladex should have EXACT confidence, got %s", item.MatchConfidence)
			}
		}
		if strings.Contains(item.Medication, "Ozempic") {
			foundOzempic = true
		}
	}

	if !foundZoladex {
		t.Error("Expected Zoladex in results")
	}
	if !foundOzempic {
		t.Error("Expected Ozempic in results")
	}
}

func TestIntegration_FuzzyMatchingWorks(t *testing.T) {
	client, mappings, _ := setupIntegrationClient(t)

	// Use a slightly misspelled medication name
	msg := &domain.RawMessage{
		ID:         "int-test-fuzzy",
		Content:    "*محتاج*\n*ديكابيبتايل*", // Slightly different spelling
		SenderName: "Test",
		GroupName:  "Integration Test",
	}

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	results, err := client.ParseMessages(ctx, []*domain.RawMessage{msg}, mappings)
	if err != nil {
		t.Fatalf("ParseMessages failed: %v", err)
	}

	if len(results) != 1 || len(results[0].Items) == 0 {
		t.Fatalf("Expected results with items")
	}

	item := results[0].Items[0]
	if !strings.Contains(item.Medication, "Decapeptyl") {
		t.Errorf("Expected Decapeptyl (fuzzy match), got %s", item.Medication)
	}
}

func TestIntegration_NoMixedArabicEnglish(t *testing.T) {
	client, mappings, _ := setupIntegrationClient(t)

	msg := &domain.RawMessage{
		ID:         "int-test-mixed",
		Content:    "*عندي*\n*ابيجونال*",
		SenderName: "Test",
		GroupName:  "Integration Test",
	}

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	results, err := client.ParseMessages(ctx, []*domain.RawMessage{msg}, mappings)
	if err != nil {
		t.Fatalf("ParseMessages failed: %v", err)
	}

	if len(results) != 1 || len(results[0].Items) == 0 {
		t.Fatalf("Expected results with items")
	}

	item := results[0].Items[0]

	// Ensure no mixed Arabic/English in output
	hasArabic := false
	hasEnglish := false
	for _, r := range item.Medication {
		if r >= 0x0600 && r <= 0x06FF {
			hasArabic = true
		}
		if (r >= 'A' && r <= 'Z') || (r >= 'a' && r <= 'z') {
			hasEnglish = true
		}
	}

	if hasArabic && hasEnglish {
		t.Errorf("Found mixed Arabic/English in medication name: %s", item.Medication)
	}
}

func TestIntegration_BatchParsing(t *testing.T) {
	client, mappings, _ := setupIntegrationClient(t)

	messages := []*domain.RawMessage{
		{ID: "batch-1", Content: "*عندي* *زولادكس*"},
		{ID: "batch-2", Content: "*محتاج* *اوزمبك*"},
		{ID: "batch-3", Content: "*متوفر* *سكسندا*"},
	}

	ctx, cancel := context.WithTimeout(context.Background(), 90*time.Second)
	defer cancel()

	results, err := client.ParseMessages(ctx, messages, mappings)
	if err != nil {
		t.Fatalf("ParseMessages failed: %v", err)
	}

	if len(results) != 3 {
		t.Errorf("Expected 3 results, got %d", len(results))
	}

	// At least some items should be parsed
	totalItems := 0
	for _, r := range results {
		totalItems += len(r.Items)
	}

	if totalItems < 3 {
		t.Errorf("Expected at least 3 items across all batches, got %d", totalItems)
	}
}
