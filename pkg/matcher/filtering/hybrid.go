package filtering

import (
	"context"

	"pharmabroker/domain/entity"
	"pharmabroker/pkg/matcher"
	"pharmabroker/pkg/matcher/similarity"
)

func HybridFilter(
	ctx context.Context,
	content string,
	allMappings []*entity.MedicationMapping,
	embedder matcher.Embedder,
	comparator similarity.Comparator,
	topK int,
) map[string]string {

	// Build full dictionary including synonyms
	fullMap := make(map[string]string)
	for _, m := range allMappings {
		fullMap[m.ArabicName] = m.EnglishName
		for _, syn := range m.Synonyms {
			fullMap[syn] = m.EnglishName
		}
	}

	// Step 1: keyword
	result := KeywordFilter(content, fullMap)

	// Step 2: vector similarity
	simMatches, err := SimilarityFilter(ctx, content, allMappings, embedder, comparator, topK)
	if err == nil {
		for k, v := range simMatches {
			result[k] = v
		}
	}

	return result
}
