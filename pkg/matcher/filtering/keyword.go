package filtering

import (
	arabicPkg "pharmabroker/pkg/arabic"
	"strings"
)

// KeywordFilter matches Arabic keys via normalized substring check.
func KeywordFilter(content string, mappings map[string]string) map[string]string {
	result := make(map[string]string)
	contentNorm := arabicPkg.NormalizeForMatching(content)

	for arabicKey, english := range mappings {
		keyNorm := arabicPkg.NormalizeForMatching(arabicKey)
		if strings.Contains(contentNorm, keyNorm) {
			result[arabicKey] = english
		}
	}

	return result
}
