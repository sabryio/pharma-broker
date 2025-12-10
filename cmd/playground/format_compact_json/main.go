package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"time"

	aiDocker "pharmabroker/ai/docker"
	"pharmabroker/domain/entity"
	"pharmabroker/pkg/config"
	"pharmabroker/pkg/matcher/filtering"
	textPkg "pharmabroker/pkg/text"
	storageGorm "pharmabroker/storage/gorm"

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

// Playground 1: Compare token usage between verbose and compact JSON formats

func main() {
	log := zerolog.New(os.Stdout).With().Timestamp().Logger()

	// Load mappings from DB (requires PostgreSQL running)
	dbCfg := &storageGorm.Config{DSN: "postgres://postgres:password@localhost:5432/pharmabroker?sslmode=disable"}
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

	fmt.Printf("=== Playground 1: Compact JSON Format ===\n")
	fmt.Printf("Total mappings: %d\n\n", len(mappings))

	// Format 1: Verbose (current)
	verboseFormat := formatVerbose(mappings)
	verboseTokens := estimateTokens(verboseFormat)

	// Format 2: Compact JSON
	compactJSON := formatCompactJSON(mappings)
	compactTokens := estimateTokens(compactJSON)

	// Format 3: Tab-separated
	tabSeparated := formatTabSeparated(mappings)
	tabTokens := estimateTokens(tabSeparated)

	// Format 4: Arrow notation
	arrowFormat := formatArrow(mappings)
	arrowTokens := estimateTokens(arrowFormat)

	// Format 5: Pipe notation
	pipeFormat := formatPipe(mappings)
	pipeTokens := estimateTokens(pipeFormat)

	fmt.Printf("%-20s | %-10s | %-10s | %-10s\n", "Format", "Bytes", "Est.Tokens", "Savings")
	fmt.Printf("%s\n", strings.Repeat("-", 60))
	fmt.Printf("%-20s | %-10d | %-10d | %-10s\n", "Verbose (current)", len(verboseFormat), verboseTokens, "-")
	fmt.Printf("%-20s | %-10d | %-10d | %-10.1f%%\n", "Compact JSON", len(compactJSON), compactTokens, savings(verboseTokens, compactTokens))
	fmt.Printf("%-20s | %-10d | %-10d | %-10.1f%%\n", "Tab-Separated", len(tabSeparated), tabTokens, savings(verboseTokens, tabTokens))
	fmt.Printf("%-20s | %-10d | %-10d | %-10.1f%%\n", "Arrow Notation", len(arrowFormat), arrowTokens, savings(verboseTokens, arrowTokens))
	fmt.Printf("%-20s | %-10d | %-10d | %-10.1f%%\n", "Pipe Notation", len(pipeFormat), pipeTokens, savings(verboseTokens, pipeTokens))

	// Now test with actual AI
	fmt.Printf("\n=== Live AI Test with Compact JSON ===\n")

	cfg := &config.DockerModelConfig{
		BaseURL:        "http://localhost:12434/engines/llama.cpp/v1",
		Model:          "ai/qwen3-vl:latest",
		RequestTimeout: 2 * time.Minute,
		MaxRetries:     1,
		RetryBaseDelay: time.Second,
	}

	client, err := aiDocker.NewClient(cfg, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create client")
	}

	msg := &entity.RawMessage{
		ID:         "compact-test",
		Content:    content,
		SenderName: "Test",
		GroupName:  "Test",
	}

	start := time.Now()
	results, err := client.ParseMessages(context.Background(), []*entity.RawMessage{msg}, filtering.MapToMappingsEntity(mappings))
	duration := time.Since(start)

	if err != nil {
		log.Fatal().Err(err).Msg("AI parsing failed")
	}

	fmt.Printf("Parsed in %v\n", duration)
	fmt.Printf("Results:\n")
	for _, res := range results {
		for i, item := range res.Items {
			fmt.Printf("  [%d] %s (Raw: %s)\n", i, item.Medication, item.MedicationRaw)
		}
	}
}

func formatVerbose(mappings map[string]string) string {
	var sb strings.Builder
	sb.WriteString("\n\n## KNOWN MEDICATION TRANSLATIONS\n")
	sb.WriteString("You MUST use these specific English names for the corresponding Arabic terms:\n")
	for arabic, english := range mappings {
		sb.WriteString(fmt.Sprintf("- \"%s\" => \"%s\"\n", arabic, english))
	}
	sb.WriteString("\n")
	return sb.String()
}

func formatCompactJSON(mappings map[string]string) string {
	jsonBytes, _ := json.Marshal(mappings)
	return fmt.Sprintf("\n## MEDICATION MAP (JSON)\n%s\n", string(jsonBytes))
}

func formatTabSeparated(mappings map[string]string) string {
	var sb strings.Builder
	sb.WriteString("\n## MEDICATION MAP\n")
	for arabic, english := range mappings {
		sb.WriteString(fmt.Sprintf("%s\t%s\n", arabic, english))
	}
	return sb.String()
}

func formatArrow(mappings map[string]string) string {
	var sb strings.Builder
	sb.WriteString("\n## MEDICATION MAP\n")
	for arabic, english := range mappings {
		sb.WriteString(fmt.Sprintf("%s→%s\n", arabic, english))
	}
	return sb.String()
}

func formatPipe(mappings map[string]string) string {
	var sb strings.Builder
	sb.WriteString("\n## MEDICATION MAP\n")
	for arabic, english := range mappings {
		sb.WriteString(fmt.Sprintf("%s|%s\n", arabic, english))
	}
	return sb.String()
}

func estimateTokens(s string) int {
	count, _ := textPkg.CountTokens(s)
	return count
}

func savings(baseline, current int) float64 {
	return float64(baseline-current) / float64(baseline) * 100
}
