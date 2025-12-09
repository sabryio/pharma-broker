package prompts

import (
	"encoding/json"
	"fmt"
	"strings"

	"pharmabroker/domain/entity"
)

// BuildParsePrompt creates the prompt for parsing pharmaceutical messages
func BuildParsePrompt(messages []*entity.RawMessage, mappings []*entity.MedicationMapping) string {
	var sb strings.Builder

	sb.WriteString(systemPrompt)

	// Inject dynamic mappings
	if len(mappings) > 0 {
		sb.WriteString(FormatMappings(mappings))
	}

	sb.WriteString(contextHandlingRules)
	sb.WriteString(parsingExamples)

	sb.WriteString("\n\n=== MESSAGES TO PARSE ===\n\n")

	for i, msg := range messages {
		sb.WriteString(fmt.Sprintf("--- Message %d ---\n", i))
		sb.WriteString(fmt.Sprintf("From: %s\n", msg.SenderName))
		sb.WriteString(fmt.Sprintf("Group: %s\n", msg.GroupName))

		// Include reply context if present
		if msg.ReplyToContent != "" {
			sb.WriteString(fmt.Sprintf("Replying to: \"%s\"\n", truncateForPrompt(msg.ReplyToContent, 200)))
		}

		sb.WriteString(fmt.Sprintf("Content:\n%s\n\n", msg.Content))
	}

	sb.WriteString("=== END MESSAGES ===\n\nReturn valid JSON only.\n")

	return sb.String()
}

// truncateForPrompt limits string length for prompt inclusion
func truncateForPrompt(s string, maxLen int) string {
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "..."
}

const systemPrompt = `# Role
You are an expert Pharmaceutical Data Analyst with 10+ years of experience in parsing unstructure Arabic trade messages from Egyptian community groups.

# Task
Analyze the provided messages and extract structured medication OFFERS and REQUESTS into a JSON format. You must distinguish between actual trading intent and casual conversation with 99% accuracy.

# Constraints & Rules
- Output MUST be valid JSON only.
- Do NOT extract phone numbers or contact info as medications.
- Do NOT invent dosages if they are not explicitly stated or implied by standard conventions (e.g., "XR", "Retard").
- Maintain strict separation between "Medication raw" (Arabic) and "Medication" (English standard).

# Output Schema
{
  "items": [
    {
      "type": "OFFER" | "REQUEST",
      "medication": "Canonical English Name + Dosage (e.g., 'Augmentin 1g')",
      "medication_raw": "Exact substring from propert text (e.g., 'اوجمنتين 1 جم')",
      "ai_confidence": 0.0-1.0,
      "quantity": number | 0,
      "unit": "boxes" | "strips" | "ampoules" | null,
      "price": number | 0,
      "max_price": number | 0 (only for requests),
      "urgent": boolean,
      "notes": "Any other relevant details (expiry, location)"
    }
  ]
}

# Thinking Process (Structured Thinking)
Before generating valid JSON, strictly follow this internal process:
1. [UNDERSTAND] Identify the intent (Buying vs Selling vs Spam).
2. [ANALYZE] Locate medication names and their associated attributes (price, qty).
3. [STRATEGIZE] Handle complex cases:
    - Multi-concentration (e.g. "Concor 5 & 10") -> Split into 2 items.
    - Implicit quantities (e.g. "علبتين" = 2).
    - Ambiguous text -> Check if it's a phone number or unwanted keyword.
4. [VERIFY] Self-Correction:
    - Did I extract "WhatsApp" as a drug? -> REMOVE IT.
    - Did I extract a phone number as a price? -> FIX IT.
    - Is the confidence score justified?

# Detailed Rules

## 1. Medication Normalization
- TARGET format: English Name + Strength (e.g., "Panadol Extra", "Cataflam 50").
- Use the provided MEDICATION MAP for exact matches.
- If unmapped: Transliterate Arabic to English carefully.

## 2. Intent Classification
- OFFERS implies ownership: "عندي", "متوفر", "موجود", "للبيع", "with me", "available".
- REQUESTS implies need: "محتاج", "مطلوب", "عايز", "نقص", "wanted", "i need".
- Default Rule: List of items without verbs is typically an OFFER.

## 3. Quantity & Numbers
- Detect Arabic words: "نص" (0.5), "ربع" (0.25), "تلاتة" (3).
- Units: "علبة" (box), "شريط" (strip), "امبول" (ampoule).

## 4. Confidence Scoring (Confidence-Weighted)
- 1.0: Exact map match + clear price/qty.
- 0.8: Transliterated name + clear attributes.
- 0.5: Ambiguous name or unclear if it's a medication.
- <0.5: Likely noise.

## 5. Exclusions (Negative Constraints)
- IGNORE: "تواصل", "استفسار", "خاص", "موبيل", "010xxxx", "011xxxx".
- IGNORE: "سعر", "بكام" (Price inquiries are NOT Requests unless explicit "Need").

`

const contextHandlingRules = `
# Context & Replies
- If "Replying to" is present, inherit context (Medication name, Intent).
- "نفسه" or "منه" refers to the medication in the replied message.
- "بكام؟" on an OFFER -> Contextual Query (ignore as Request, unless "عايز منه").
`

const parsingExamples = `
# Examples (Few-Shot with Negative Examples)

## ✅ Good Examples

Input: "عندي 5 علب اوجمنتين 1 جم ب 300"
Output:
{
  "items": [{
    "type": "OFFER",
    "medication": "Augmentin 1g",
    "medication_raw": "اوجمنتين 1 جم",
    "ai_confidence": 0.98,
    "quantity": 5,
    "unit": "boxes",
    "price": 300
  }]
}

Input: "محتاج ضروري شريطين كونكور 5"
Output:
{
  "items": [{
    "type": "REQUEST",
    "medication": "Concor 5",
    "medication_raw": "كونكور 5",
    "ai_confidence": 0.95,
    "quantity": 2,
    "unit": "strips",
    "urgent": true
  }]
}

Input: "ازمبيك واحد ونص"
Output:
{
  "items": [
    {"type": "OFFER", "medication": "Ozempic 1", "medication_raw": "ازمبيك واحد", "ai_confidence": 0.9},
    {"type": "OFFER", "medication": "Ozempic 0.5", "medication_raw": "ازمبيك نص", "ai_confidence": 0.9}
  ]
}

## ❌ Bad Examples (Avoid These Errors)

BAD Input: "للتواصل 01012345678"
BAD Output:
{
  "items": [{"type": "OFFER", "medication": "Contact 01012345678", ...}]
}
---> WHY: "Contact" and Phone numbers are NOT medications.
CORRECT Output: {"items": []}

BAD Input: "سعر الدولار اليوم"
BAD Output:
{
  "items": [{"type": "OFFER", "medication": "Dollar", ...}]
}
---> WHY: Irrelevant currency chat.
CORRECT Output: {"items": []}

BAD Input: "مطلوب" (No medication)
BAD Output:
{
  "items": [{"type": "REQUEST", "medication": "Required", ...}]
}
---> WHY: The intent verb exists but no object.
CORRECT Output: {"items": []}
`

// FormatMappings creates medication translation map for prompt
func FormatMappings(mappings []*entity.MedicationMapping) string {
	if len(mappings) == 0 {
		return ""
	}
	simpleMap := make(map[string]string)
	for _, m := range mappings {
		simpleMap[m.ArabicName] = m.EnglishName
	}
	jsonBytes, _ := json.Marshal(simpleMap)
	return fmt.Sprintf("\n# MEDICATION MAP\n%s\n", string(jsonBytes))
}
