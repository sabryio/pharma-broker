package main

import (
	"context"
	"fmt"
	"maps"
	"math"
	"os"
	"sort"
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

// Playground 3: Vector Similarity Search (RAG)
// Uses embeddings to find semantically similar medication mappings

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

	fmt.Printf("=== Playground 3: Vector Similarity Search ===\n")
	fmt.Printf("Total mappings in DB: %d\n\n", len(allMappings))

	// Setup AI client for embeddings
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

	// Step 1: Check if embeddings exist, if not, generate them
	fmt.Println("Step 1: Checking/generating embeddings...")
	needsEmbedding := 0
	for _, m := range allMappings {
		if len(m.Embedding) == 0 {
			needsEmbedding++
		}
	}

	if needsEmbedding > 0 {
		fmt.Printf("  Generating embeddings for %d mappings...\n", needsEmbedding)

		// Batch embed
		arabicNames := make([]string, 0, needsEmbedding)
		indexMap := make(map[int]int) // position in arabicNames -> index in allMappings

		for i, m := range allMappings {
			if len(m.Embedding) == 0 {
				indexMap[len(arabicNames)] = i
				arabicNames = append(arabicNames, m.ArabicName)
			}
		}

		embeddings, err := client.EmbedBatch(context.Background(), arabicNames)
		if err != nil {
			log.Fatal().Err(err).Msg("Failed to generate embeddings")
		}

		// Store embeddings
		for j, emb := range embeddings {
			idx := indexMap[j]
			allMappings[idx].Embedding = emb
			if err := repo.Save(context.Background(), allMappings[idx]); err != nil {
				log.Error().Err(err).Str("name", allMappings[idx].ArabicName).Msg("Failed to save embedding")
			}
		}
		fmt.Printf("  ✓ Generated and stored %d embeddings\n", len(embeddings))
	} else {
		fmt.Println("  ✓ All mappings already have embeddings")
	}

	// Step 2: Test semantic search
	fmt.Println("\nStep 2: Testing semantic similarity search...")

	fmt.Printf("Test message:\n%s\n\n", content)

	// Embed the test message
	messageEmbedding, err := client.Embed(context.Background(), content)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to embed message")
	}

	// Find top-K similar mappings
	topK := 10
	similar := findSimilar(allMappings, messageEmbedding, topK)

	fmt.Printf("Top %d similar mappings (by cosine similarity):\n", topK)
	for i, s := range similar {
		fmt.Printf("  %2d. [%.4f] %s => %s\n", i+1, s.score, s.mapping.ArabicName, s.mapping.EnglishName)
	}

	// Step 3: Compare with keyword matching
	fmt.Println("\nStep 3: Comparison with keyword matching...")

	keywordMatches := make(map[string]string)
	for _, m := range allMappings {
		if strings.Contains(strings.ToLower(content), strings.ToLower(m.ArabicName)) {
			keywordMatches[m.ArabicName] = m.EnglishName
		}
	}

	fmt.Printf("Keyword matches: %d\n", len(keywordMatches))
	for arabic, english := range keywordMatches {
		fmt.Printf("  ✓ %s => %s\n", arabic, english)
	}

	semanticOnly := make(map[string]string)
	for _, s := range similar {
		if _, exists := keywordMatches[s.mapping.ArabicName]; !exists {
			semanticOnly[s.mapping.ArabicName] = s.mapping.EnglishName
		}
	}

	fmt.Printf("\nSemantic-only matches (not found by keywords): %d\n", len(semanticOnly))
	for arabic, english := range semanticOnly {
		fmt.Printf("  + %s => %s\n", arabic, english)
	}

	// Step 4: Live AI test with semantic filtering
	fmt.Println("\n=== Live AI Test with Semantic-Filtered Mappings ===")

	filteredMappings := make(map[string]string)
	// Combine keyword + semantic
	maps.Copy(filteredMappings, keywordMatches)
	for _, s := range similar[:min(5, len(similar))] { // Top 5 semantic
		filteredMappings[s.mapping.ArabicName] = s.mapping.EnglishName
	}

	msg := &domain.RawMessage{
		ID:         "vector-search-test",
		Content:    content,
		SenderName: "Test",
		GroupName:  "Test",
	}

	start := time.Now()
	results, err := client.ParseMessages(context.Background(), []*domain.RawMessage{msg}, ai.MapToMedicationMappings(filteredMappings))
	duration := time.Since(start)

	if err != nil {
		log.Fatal().Err(err).Msg("AI parsing failed")
	}

	fmt.Printf("Parsed in %v (with %d filtered mappings)\n", duration, len(filteredMappings))
	fmt.Printf("Results:\n")
	for _, res := range results {
		for i, item := range res.Items {
			fmt.Printf("  [%d] %s (Raw: %s) [%s]\n", i, item.Medication, item.MedicationRaw, item.Type)
		}
	}
}

type scoredMapping struct {
	mapping *domain.MedicationMapping
	score   float32
}

func findSimilar(mappings []*domain.MedicationMapping, queryEmbedding []float32, topK int) []scoredMapping {
	var results []scoredMapping

	for _, m := range mappings {
		if len(m.Embedding) == 0 {
			continue
		}
		score := cosineSimilarity(queryEmbedding, m.Embedding)
		results = append(results, scoredMapping{m, score})
	}

	// Sort by score descending
	sort.Slice(results, func(i, j int) bool {
		return results[i].score > results[j].score
	})

	if len(results) > topK {
		return results[:topK]
	}
	return results
}

func cosineSimilarity(a, b []float32) float32 {
	if len(a) != len(b) {
		return 0
	}

	var dot, normA, normB float32
	for i := range a {
		dot += a[i] * b[i]
		normA += a[i] * a[i]
		normB += b[i] * b[i]
	}

	if normA == 0 || normB == 0 {
		return 0
	}

	return dot / (float32(math.Sqrt(float64(normA))) * float32(math.Sqrt(float64(normB))))
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
