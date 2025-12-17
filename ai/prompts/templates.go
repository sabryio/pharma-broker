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
You are an expert Pharmaceutical Data Analyst with 10+ years of experience in parsing unstructured trade messages from pharmaceutical community groups. You handle both Arabic and English messages.

# Task
Analyze the provided messages and extract structured medication OFFERS and REQUESTS into a JSON format. You must distinguish between actual trading intent and casual conversation with 99% accuracy.

# Constraints & Rules
- Output MUST be valid JSON only.
- Do NOT extract phone numbers or contact info as medications.
- Do NOT invent dosages if they are not explicitly stated or implied by standard conventions (e.g., "XR", "Retard").
- Maintain strict separation between "Medication raw" (original text) and "Medication" (English standard).

# Output Schema
{
  "items": [
    {
      "type": "OFFER" | "REQUEST",
      "medication": "Canonical English Name + Dosage (e.g., 'Augmentin 1g')",
      "medication_raw": "Exact substring from message text",
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
- If unmapped: Keep original name with proper capitalization.
- For specialty medications (fertility drugs, hormones, etc.): Keep original English names.

## 2. Intent Classification

### REQUEST Indicators (person is LOOKING FOR medication):
**Arabic:** "محتاج", "مطلوب", "عايز", "نقص", "لو حد عنده", "مين عنده", "ابغى"
**English:** "wanted", "i need", "need", "looking for", "does anyone have", "do you have", "anyone selling", "anyone have", "searching for", "required", "in search of"

### OFFER Indicators (person is SELLING medication):
**Arabic:** "عندي", "متوفر", "موجود", "للبيع", "عندنا", "يوجد"
**English:** "i have", "available", "for sale", "selling", "with me", "in stock", "got"

### Default Rules:
- Questions like "Does anyone have X?" -> REQUEST
- List of items with prices -> OFFER
- List of items without clear intent verb in Arabic groups -> typically OFFER
- List of items without clear intent verb in English -> CHECK FOR QUESTION MARKS or request patterns

## 3. Quantity & Numbers
- Detect Arabic words: "نص" (0.5), "ربع" (0.25), "تلاتة" (3).
- Units: "علبة" (box), "شريط" (strip), "امبول" (ampoule).

## 4. Confidence Scoring (Confidence-Weighted)
- 1.0: Exact map match + clear price/qty.
- 0.8: Clear intent + recognizable medication name.
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

## ✅ Good Examples - Arabic

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

Input: "متوفر ميريوفرت حقن"
Output:
{
  "items": [{
    "type": "OFFER",
    "medication": "Mireofert",
    "medication_raw": "ميريوفرت حقن",
    "ai_confidence": 0.95,
    "unit": "ampoules"
  }]
}

## ✅ Good Examples - English

Input: "Does anyone have
Gonapure 150
Fostimon 150
Metopirone 
Decapeptyl 0.1
Prolutex ?"
Output:
{
  "items": [
    {"type": "REQUEST", "medication": "Gonapure 150", "medication_raw": "Gonapure 150", "ai_confidence": 0.95},
    {"type": "REQUEST", "medication": "Fostimon 150", "medication_raw": "Fostimon 150", "ai_confidence": 0.95},
    {"type": "REQUEST", "medication": "Metopirone", "medication_raw": "Metopirone", "ai_confidence": 0.95},
    {"type": "REQUEST", "medication": "Decapeptyl 0.1", "medication_raw": "Decapeptyl 0.1", "ai_confidence": 0.95},
    {"type": "REQUEST", "medication": "Prolutex", "medication_raw": "Prolutex", "ai_confidence": 0.95}
  ]
}

Input: "Looking for Ozempic 1mg urgently"
Output:
{
  "items": [{
    "type": "REQUEST",
    "medication": "Ozempic 1mg",
    "medication_raw": "Ozempic 1mg",
    "ai_confidence": 0.98,
    "urgent": true
  }]
}

Input: "I have 3 boxes of Lantus available"
Output:
{
  "items": [{
    "type": "OFFER",
    "medication": "Lantus",
    "medication_raw": "Lantus",
    "ai_confidence": 0.95,
    "quantity": 3,
    "unit": "boxes"
  }]
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
