package ai

import (
	"fmt"
	"strings"

	"github.com/pkoukk/tiktoken-go"
)

// CountTokens estimates the number of tokens in a string for a given model.
// If the model is not directly supported by tiktoken, it defaults to "gpt-4" (cl100k_base).
func CountTokens(model string, text string) (int, error) {
	// Standardize/fallback for local models (e.g. "ai/qwen3-vl:latest")
	targetModel := model
	if strings.Contains(model, "qwen") || strings.Contains(model, "llama") || strings.Contains(model, "/") {
		targetModel = "gpt-4"
	}

	tke, err := tiktoken.EncodingForModel(targetModel)
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
