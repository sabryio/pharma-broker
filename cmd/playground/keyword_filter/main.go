package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"time"

	"pharmabroker/internal/ai"
	"pharmabroker/internal/config"
	"pharmabroker/internal/domain"
	"pharmabroker/internal/storage"

	"github.com/rs/zerolog"
)

// Test Content
const content string = `*محتاج جدا*
 
*اوزمبك واحد ونص وربع*
_____________________

*مريوفيرت 150*
*فوستيمون 150*

*جونابيور 150*
*جونابيور 75*

*سيتروتايد ربع*

*أوفتريل 250*
*كوريومون 5000*
*ابيفاسي 5000*

*ديكابيبتايل*
*تريبتوفيم*

*جونال 900*

*ابيجونال 75*

*زولادكس 3.6*

*برولوتكس*
_____________________

*سكسندا*

*ريبلسس 7*

*جوناتستون حقن*

*انفانز*

*بنتازا اقراص*

*بنتازا لبوس*

*زيلودا* *(علبة مستوردة ناقصها شريط ب ٤٧٠٠)*`

// Playground 2: Keyword-Based Filtering
// Only inject mappings where the Arabic key appears in the message content

func main() {
	log := zerolog.New(os.Stdout).With().Timestamp().Logger()

	// Load mappings from DB
	dbCfg := &config.DatabaseConfig{Path: "data/pharmabroker.db"}
	gormDB, err := storage.NewGormDB(dbCfg)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to open DB")
	}
	repo := storage.NewGormMedicationMappingRepo(gormDB)
	allMappings, _ := repo.GetAll(context.Background())

	fullMappings := make(map[string]string)
	for _, m := range allMappings {
		fullMappings[m.ArabicName] = m.EnglishName
	}

	fmt.Printf("=== Playground 2: Keyword-Based Filtering ===\n")
	fmt.Printf("Total mappings available: %d\n\n", len(fullMappings))

	fmt.Printf("Message content:\n%s\n\n", content)

	// Filter mappings based on keyword matching
	filteredMappings := filterByKeyword(content, fullMappings)

	fmt.Printf("Filtered mappings (keywords found in message): %d\n", len(filteredMappings))
	for arabic, english := range filteredMappings {
		fmt.Printf("  ✓ %s => %s\n", arabic, english)
	}

	// Compare token usage
	fullJSON, _ := json.Marshal(fullMappings)
	filteredJSON, _ := json.Marshal(filteredMappings)

	fmt.Printf("\n%-25s | %-10s | %-10s\n", "Approach", "Mappings", "JSON Bytes")
	fmt.Printf("%s\n", strings.Repeat("-", 50))
	fmt.Printf("%-25s | %-10d | %-10d\n", "Full Injection", len(fullMappings), len(fullJSON))
	fmt.Printf("%-25s | %-10d | %-10d\n", "Keyword Filtered", len(filteredMappings), len(filteredJSON))
	fmt.Printf("%-25s | %-10s | %-10.1f%%\n", "Savings", "-",
		float64(len(fullJSON)-len(filteredJSON))/float64(len(fullJSON))*100)

	// Test with AI
	fmt.Printf("\n=== Live AI Test with Filtered Mappings ===\n")

	cfg := &config.DockerModelConfig{
		BaseURL:        "http://localhost:12434/engines/llama.cpp/v1",
		Model:          "ai/qwen3-vl:latest",
		RequestTimeout: 2 * time.Minute,
		MaxRetries:     1,
		RetryBaseDelay: time.Second,
	}

	client, err := ai.NewDockerModelClient(cfg, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create client")
	}

	msg := &domain.RawMessage{
		ID:         "keyword-filter-test",
		Content:    content,
		SenderName: "Test",
		GroupName:  "Test",
	}

	start := time.Now()
	results, err := client.ParseMessages(context.Background(), []*domain.RawMessage{msg}, filteredMappings)
	duration := time.Since(start)

	if err != nil {
		log.Fatal().Err(err).Msg("AI parsing failed")
	}

	fmt.Printf("Parsed in %v\n", duration)
	fmt.Printf("Results:\n")
	for _, res := range results {
		for i, item := range res.Items {
			status := "✓"
			// Check if mapping was applied correctly
			if expectedEnglish, ok := filteredMappings[extractArabicRoot(item.MedicationRaw)]; ok {
				if !strings.Contains(item.Medication, expectedEnglish) {
					status = "✗ MISMATCH"
				}
			}
			fmt.Printf("  [%d] %s %s (Raw: %s)\n", i, status, item.Medication, item.MedicationRaw)
		}
	}
}

// filterByKeyword returns only mappings where the Arabic key appears in the content
func filterByKeyword(content string, mappings map[string]string) map[string]string {
	result := make(map[string]string)
	contentLower := strings.ToLower(content)

	for arabic, english := range mappings {
		// Check if Arabic key exists in message
		if strings.Contains(contentLower, strings.ToLower(arabic)) {
			result[arabic] = english
		}
	}

	return result
}

// extractArabicRoot attempts to get the base Arabic medication name from raw text
func extractArabicRoot(raw string) string {
	// Remove numbers and common suffixes
	parts := strings.Fields(raw)
	if len(parts) > 0 {
		return parts[0]
	}
	return raw
}
