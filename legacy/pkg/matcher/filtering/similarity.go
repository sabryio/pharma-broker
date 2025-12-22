package filtering

import (
	"context"
	"sort"

	"pharmabroker/domain/entity"
	"pharmabroker/pkg/matcher"
	"pharmabroker/pkg/matcher/similarity"
)

func SimilarityFilter(
	ctx context.Context,
	content string,
	mappings []*entity.MedicationMapping,
	embedder matcher.Embedder,
	comparator similarity.Comparator,
	topK int,
) (map[string]string, error) {

	if len(mappings) == 0 || topK <= 0 {
		return make(map[string]string), nil
	}

	contentVec, err := embedder.Embed(ctx, content)
	if err != nil {
		return nil, err
	}

	type scored struct {
		m *entity.MedicationMapping
		s float32
	}

	var list []scored
	for _, m := range mappings {
		if len(m.Embedding) == 0 {
			continue
		}
		score := comparator.Similarity(contentVec, m.Embedding)
		list = append(list, scored{m, score})
	}

	sort.Slice(list, func(i, j int) bool {
		return list[i].s > list[j].s
	})

	result := make(map[string]string)
	for i := 0; i < topK && i < len(list); i++ {
		item := list[i]
		result[item.m.ArabicName] = item.m.EnglishName
	}

	return result, nil
}
