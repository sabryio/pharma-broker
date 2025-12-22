package text

import "strings"

// ContainsIgnoreCase checks whether s contains substr, case insensitive.
func ContainsIgnoreCase(s, substr string) bool {
	return strings.Contains(
		strings.ToUpper(s),
		strings.ToUpper(substr),
	)
}
