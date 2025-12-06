package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"time"

	"github.com/invopop/jsonschema"
	"github.com/joho/godotenv"
	"github.com/openai/openai-go"
	"github.com/openai/openai-go/option"
)

// Define the struct we want the LLM to output
type TestOutput struct {
	Medication string  `json:"medication" jsonschema_description:"Name of the medication"`
	Quantity   float64 `json:"quantity" jsonschema_description:"Quantity requested"`
	IsUrgent   bool    `json:"is_urgent" jsonschema_description:"Whether the request is urgent"`
}

func GenerateSchema[T any]() any {
	reflector := jsonschema.Reflector{
		AllowAdditionalProperties: false,
		DoNotReference:            true,
	}
	var v T
	return reflector.Reflect(v)
}

var TestOutputSchema = GenerateSchema[TestOutput]()

func main() {
	_ = godotenv.Load()

	// Hardcoded config for local test
	baseURL := "http://localhost:12434/engines/llama.cpp/v1"
	model := "ai/qwen3-vl:latest" // Or whatever is running

	if url := os.Getenv("PHARMA_DOCKER_BASE_URL"); url != "" {
		baseURL = url
	}
	if m := os.Getenv("PHARMA_DOCKER_MODEL"); m != "" {
		model = m
	}

	fmt.Printf("Testing Structured Outputs against:\nURL: %s\nModel: %s\n", baseURL, model)

	client := openai.NewClient(
		option.WithBaseURL(baseURL),
		option.WithAPIKey("not-needed"),
	)

	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()

	schemaParam := openai.ResponseFormatJSONSchemaJSONSchemaParam{
		Name:        "medication_request",
		Description: openai.String("Extract medication details"),
		Schema:      TestOutputSchema,
		Strict:      openai.Bool(true),
	}

	fmt.Println("Sending request...")
	start := time.Now()

	chat, err := client.Chat.Completions.New(ctx, openai.ChatCompletionNewParams{
		Messages: []openai.ChatCompletionMessageParamUnion{
			openai.UserMessage("I need 2 boxes of Augmentin urgently!"),
		},
		ResponseFormat: openai.ChatCompletionNewParamsResponseFormatUnion{
			OfJSONSchema: &openai.ResponseFormatJSONSchemaParam{
				JSONSchema: schemaParam,
			},
		},
		Model: model,
	})

	if err != nil {
		fmt.Printf("❌ API Error: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("✅ Response received in %v\n", time.Since(start))
	fmt.Printf("Raw Content: %s\n", chat.Choices[0].Message.Content)

	var result TestOutput
	if err := json.Unmarshal([]byte(chat.Choices[0].Message.Content), &result); err != nil {
		fmt.Printf("❌ JSON Parse Error: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("Parsed Struct: %+v\n", result)
}
