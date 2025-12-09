package ai

import (
	"fmt"

	"github.com/pkoukk/tiktoken-go"
)

// CountTokens estimates the number of tokens in a string for a given model.
// If the model is not directly supported by tiktoken, it defaults to "gpt-4" (cl100k_base).
func CountTokens(text string) (int, error) {
	// Standardize/fallback for local models (e.g. "ai/qwen3-vl:latest")
	tke, err := tiktoken.EncodingForModel("gpt-4")
	if err != nil {
		// Fallback to cl100k_base directly if model lookup fails
		tke, err = tiktoken.GetEncoding("cl100k_base")
		if err != nil {
			return 0, fmt.Errorf("failed to get encoding: %v", err)
		}
	}

	tokens := tke.Encode(text, nil, nil)
	return len(tokens), nil
}
