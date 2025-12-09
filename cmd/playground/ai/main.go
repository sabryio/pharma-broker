// Playground to test Docker Model Runner parsing
//
// Usage:
//   go run cmd/playground/main.go                     # Use default messages
//   go run cmd/playground/main.go -input messages.json # Load from file
//   go run cmd/playground/main.go -output results.json # Save results to file

package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/joho/godotenv"
	"github.com/rs/zerolog"

	"pharmabroker/ai"
	"pharmabroker/domain/entity"
	"pharmabroker/pkg/config"
)

// TestMessage represents a message loaded from JSON
type TestMessage struct {
	ID        string `json:"id"`
	GroupJID  string `json:"group_jid"`
	GroupName string `json:"group_name"`
	Content   string `json:"content"`
}

// TestResult represents the output result
type TestResult struct {
	MessageID   string              `json:"message_id"`
	Content     string              `json:"content"`
	ParsedItems []entity.ParsedItem `json:"parsed_items"`
	Error       string              `json:"error,omitempty"`
	ElapsedMs   int64               `json:"elapsed_ms"`
}

// TestSummary represents the full test output
type TestSummary struct {
	Provider  string       `json:"provider"`
	Model     string       `json:"model"`
	TotalTime string       `json:"total_time"`
	Results   []TestResult `json:"results"`
	TestedAt  string       `json:"tested_at"`
}

func main() {
	// Parse flags
	inputFile := flag.String("input", "", "JSON file with test messages")
	outputFile := flag.String("output", "", "JSON file to save results")
	flag.Parse()

	// Load .env
	_ = godotenv.Load()

	// Setup logging
	log := zerolog.New(zerolog.ConsoleWriter{Out: os.Stdout}).
		With().
		Timestamp().
		Logger()

	// Load config
	cfg := config.Load()
	cfg.AI.Provider = "docker"

	log.Info().
		Str("provider", cfg.AI.Provider).
		Str("model", cfg.DockerModel.Model).
		Msg("Testing Docker Model Runner")

	// Create AI provider
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Minute)
	defer cancel()

	provider, err := ai.NewAIProvider(ctx, cfg, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create AI provider")
	}

	// Load or create test messages
	var testMessages []*entity.RawMessage
	if *inputFile != "" {
		testMessages, err = loadMessagesFromFile(*inputFile)
		if err != nil {
			log.Fatal().Err(err).Str("file", *inputFile).Msg("Failed to load messages")
		}
		log.Info().Int("count", len(testMessages)).Str("file", *inputFile).Msg("Loaded messages from file")
	} else {
		log.Info().Msg("No input file specified")
		os.Exit(1)
	}

	// Run test
	separator := strings.Repeat("=", 60)
	fmt.Println("\n" + separator)
	fmt.Println("PARSING TEST")
	fmt.Println(separator)

	// Load medication mappings for testing
	commonMedications, err := entity.LoadRichMedicationMappings("medications.json")
	if err != nil {
		log.Warn().Err(err).Msg("Failed to load medications.json")
	}

	start := time.Now()
	results, err := provider.ParseMessages(ctx, testMessages, commonMedications)
	elapsed := time.Since(start)

	if err != nil {
		log.Error().Err(err).Msg("Parsing failed")
		os.Exit(1)
	}

	log.Info().Dur("elapsed", elapsed).Int("results", len(results)).Msg("Parsing completed")

	// Build test results
	var testResults []TestResult
	for i, result := range results {
		tr := TestResult{
			MessageID: testMessages[i].ID,
			Content:   testMessages[i].Content,
			ElapsedMs: elapsed.Milliseconds() / int64(len(results)),
		}
		if result.Error != "" {
			tr.Error = result.Error
		}
		tr.ParsedItems = result.Items
		testResults = append(testResults, tr)
	}

	// Print results
	for i, result := range results {
		fmt.Printf("\n--- Message %d (ID: %s) ---\n", i+1, testMessages[i].ID)
		fmt.Printf("Content:\n%s\n", testMessages[i].Content)

		if result.Error != "" {
			fmt.Printf("\n❌ ERROR: %s\n", result.Error)
			continue
		}

		fmt.Printf("\n✅ Parsed %d items:\n", len(result.Items))
		for j, item := range result.Items {
			fmt.Printf("  %d. [%s] %s", j+1, item.Type, item.Medication)
			if item.Quantity > 0 {
				unit := ""
				if item.Unit != nil {
					unit = *item.Unit
				}
				fmt.Printf(" (qty: %f %s)", item.Quantity, unit)
			}
			if item.Price > 0 {
				fmt.Printf(" @ %.0f", item.Price)
			}
			if item.Urgent {
				fmt.Printf(" ⚠️URGENT")
			}
			fmt.Println()
		}
	}

	// Save results if output file specified
	if *outputFile != "" {
		summary := TestSummary{
			Provider:  cfg.AI.Provider,
			Model:     cfg.DockerModel.Model,
			TotalTime: elapsed.String(),
			Results:   testResults,
			TestedAt:  time.Now().Format(time.RFC3339),
		}
		if err := saveResultsToFile(*outputFile, summary); err != nil {
			log.Error().Err(err).Str("file", *outputFile).Msg("Failed to save results")
		} else {
			log.Info().Str("file", *outputFile).Msg("Results saved to file")
		}
	}

	fmt.Println("\n" + separator)
	fmt.Printf("✅ Completed in %v\n", elapsed)
}

func loadMessagesFromFile(path string) ([]*entity.RawMessage, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	var testMsgs []TestMessage
	if err := json.Unmarshal(data, &testMsgs); err != nil {
		return nil, err
	}

	var messages []*entity.RawMessage
	for _, tm := range testMsgs {
		messages = append(messages, &entity.RawMessage{
			ID:        tm.ID,
			GroupJID:  tm.GroupJID,
			GroupName: tm.GroupName,
			Content:   tm.Content,
			Timestamp: time.Now(),
		})
	}
	return messages, nil
}

func saveResultsToFile(path string, summary TestSummary) error {
	data, err := json.MarshalIndent(summary, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, data, 0644)
}
