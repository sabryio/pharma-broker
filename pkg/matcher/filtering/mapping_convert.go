package filtering

import "pharmabroker/domain/entity"

func MapToMappingsEntity(src map[string]string) []*entity.MedicationMapping {
	out := make([]*entity.MedicationMapping, 0, len(src))
	for arabic, english := range src {
		out = append(out, &entity.MedicationMapping{
			ArabicName:  arabic,
			EnglishName: english,
		})
	}
	return out
}

func MappingsEntityToMap(list []*entity.MedicationMapping) map[string]string {
	out := make(map[string]string)
	for _, m := range list {
		out[m.ArabicName] = m.EnglishName
	}
	return out
}
