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

func main() {
	log := zerolog.New(os.Stdout).With().Timestamp().Logger()

	// 1. Initialize Real Database
	// We assume this script is run from the project root.
	dbPath := "data/pharmabroker.db"
	if _, err := os.Stat(dbPath); os.IsNotExist(err) {
		log.Fatal().Str("path", dbPath).Msg("Database file not found. Please run from project root.")
	}

	dbCfg := &config.DatabaseConfig{
		Path: dbPath,
	}

	gormDB, err := storage.NewGormDB(dbCfg)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to initialize database connection")
	}

	// 2. Fetch Mappings from DB
	repo := storage.NewGormMedicationMappingRepo(gormDB)
	allMappings, err := repo.GetAll(context.Background())
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to fetch mappings from DB")
	}

	// Seed if empty
	if len(allMappings) == 0 {
		log.Info().Msg("Database mappings table is empty. Seeding from medications.json...")
		seedData, err := os.ReadFile("medications.json")
		if err != nil {
			log.Fatal().Err(err).Msg("Failed to read medications.json for seeding")
		}

		var jsonMap map[string]string
		if err := json.Unmarshal(seedData, &jsonMap); err != nil {
			log.Fatal().Err(err).Msg("Failed to parse medications.json")
		}

		count := 0
		for k, v := range jsonMap {
			mapping := &domain.MedicationMapping{
				ArabicName:  k,
				EnglishName: v,
			}
			if err := repo.Save(context.Background(), mapping); err != nil {
				log.Error().Err(err).Str("key", k).Msg("Failed to save mapping to DB")
			}
			count++
		}
		log.Info().Int("seeded_count", count).Msg("Seeding complete")

		// Re-fetch
		allMappings, err = repo.GetAll(context.Background())
		if err != nil {
			log.Fatal().Err(err).Msg("Failed to re-fetch mappings after seeding")
		}
	}

	// Transform to map[string]string for AI client
	mappings := make(map[string]string)
	for _, m := range allMappings {
		mappings[m.ArabicName] = m.EnglishName
		// Add synonyms
		for _, syn := range m.Synonyms {
			mappings[syn] = m.EnglishName
		}
	}

	log.Info().Int("total_keys", len(mappings)).Msg("Loaded medication mappings from Database")

	// 3. Setup AI Client
	cfg := &config.DockerModelConfig{
		BaseURL:            "http://localhost:12434/engines/llama.cpp/v1",
		Model:              "ai/qwen3-vl:latest",
		RequestTimeout:     2 * time.Minute,
		MaxRetries:         1,
		RetryBaseDelay:     time.Second,
		EmbeddingModelName: "ai/embeddinggemma",
	}

	var sb strings.Builder
	write := func(s string) {
		sb.WriteString(s)
		sb.WriteString("\n")
		fmt.Println(s)
	}

	write("Initializing Docker Model Client...")
	client, err := ai.NewDockerModelClient(cfg, log)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create client")
	}

	// Enable hybrid filtering (keyword + vector)
	client.SetMappings(allMappings)

	msg := &domain.RawMessage{
		ID:         "test-repro-db",
		Content:    content,
		SenderName: "Test User",
		GroupName:  "Test Group",
	}

	write("Sending message to AI...")
	start := time.Now()
	results, err := client.ParseMessages(context.Background(), []*domain.RawMessage{msg}, ai.MapToMedicationMappings(mappings))
	duration := time.Since(start)

	if err != nil {
		log.Fatal().Err(err).Msg("Failed to parse messages")
	}

	write(fmt.Sprintf("Parsed in %v", duration))
	write("=== Results ===")

	for _, res := range results {
		if res.Error != "" {
			write(fmt.Sprintf("ERROR: %s", res.Error))
		}
		for i, item := range res.Items {
			write(fmt.Sprintf("[%d] %-20s (Raw: %-20s) [Type: %s]",
				i, item.Medication, item.MedicationRaw, item.Type))

			// Check for Xeloda (زيلودا)
			if item.MedicationRaw == "زيلودا" && item.Medication == "Xeloda" {
				write(">>> SUCCESS: Xeloda mapping enforced correctly via DB!")
			}
			// Check for Zoladex (زولادكس)
			if item.MedicationRaw == "زولادكس 3.6" && strings.Contains(item.Medication, "Zoladex") {
				write(">>> SUCCESS: Zoladex mapping correct.")
			}

			// Check Cetrotide
			if strings.Contains(item.MedicationRaw, "سيتروتايد") && strings.Contains(item.Medication, "Cetrotide") {
				write(">>> SUCCESS: Cetrotide mapping correct.")
			}
		}
	}

	outputFile, err := os.Create("results.txt")
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to create results file")
	}
	defer outputFile.Close()

	outputFile.WriteString(sb.String())
}
