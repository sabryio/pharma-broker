package ai

import (
	"encoding/json"
	"fmt"
	"strings"

	"pharmabroker/internal/domain"
)

// buildParsePrompt creates the prompt for parsing pharmaceutical messages
func buildParsePrompt(messages []*domain.RawMessage, mappings []*domain.MedicationMapping) string {
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
You are a pharmaceutical message parser for Egyptian WhatsApp trading groups.

# Task  
Extract medication OFFERS and REQUESTS from Arabic messages into structured JSON.

# Output Schema
{
  "items": [
    {
      "type": "OFFER" or "REQUEST",
      "medication": "English name + dosage (e.g., Zoladex 3.6)",
      "medication_raw": "Exact Arabic text with numbers",
      "ai_confidence": 0.0-1.0,
      "quantity": number or 0,
      "unit": "boxes/strips/ampoules" or null,
      "price": number or 0,
      "max_price": number or 0 (for requests),
      "urgent": boolean,
      "notes": "extra details"
    }
  ]
}

# Core Rules (Priority Order)

## 1. Medication Field Format
- MUST be fully English (no Arabic characters)
- MUST include dosage: "Zoladex 3.6" not "Zoladex"
- Use MEDICATION MAP below if Arabic name matches
- If not in map: transliterate entire word to English

Examples:
  زولادكس 3.6 → medication: "Zoladex 3.6"
  ريبلسس 7 → medication: "Rybelsus 7"  
  ابيجونال 75 → medication: "Epigonal 75"
  جونال 900 → medication: "Gonal-F 900"

## 2. Medication Raw Field  
Copy EXACT Arabic text including all numbers:
  زولادكس 3.6 → medication_raw: "زولادكس 3.6"

## 3. Multi-Concentration Pattern
When multiple dosages listed together, create SEPARATE items:
  "اوزمبك واحد ونص وربع" = 3 items:
    - Ozempic 1 (واحد = 1)
    - Ozempic 0.5 (نص = 0.5)
    - Ozempic 0.25 (ربع = 0.25)

Arabic dosage words:
  واحد = 1
  نص/ونص = 0.5 (half)
  ربع/وربع = 0.25 (quarter)

## 4. Message Type Classification
- OFFER: عندي، متوفر، للبيع، available، موجود
- REQUEST: محتاج، عايز، مطلوب، wanted، need
- Default: Lists without context → OFFER
- Empty/chat → return {"items": []}

## 5. Quantity & Price
- Extract units: علبة (box), شريط (strip), امبول (ampoule)
- Currency is EGP: ج.م، جنيه، EGP، LE

## 6. Urgency
Mark urgent=true if: ضروري، عاجل، urgent، ASAP

## 7. Confidence Scoring
- 0.8-1.0: Exact map match, clear extraction
- 0.5-0.79: Spelling variation, unclear quantity
- 0.0-0.49: Unknown medication, heavy transliteration

## 8. Exclusions
Do NOT extract as medications:
  واتس، فون، تواصل، موبيل، WhatsApp، Phone، Contact

`

const contextHandlingRules = `
# Reply Context Handling
When "Replying to:" appears, use quoted message for context:
- "نفس السعر" → use price from quote
- "نفس الكمية" → use quantity from quote
- "متوفر"/"عندي" replying to REQUEST → OFFER for that medication
- "محتاج" replying to OFFER → REQUEST for that medication
- Price reply "300" → extract price for medication in context

`

const parsingExamples = `
# Examples

Input: "عندي اوجمنتين 1 جم ٥ علب ب ٣٠٠ جنيه"
Output:
{
  "items": [{
    "type": "OFFER",
    "medication": "Augmentin 1g",
    "medication_raw": "اوجمنتين 1 جم",
    "ai_confidence": 0.95,
    "quantity": 5,
    "unit": "boxes",
    "price": 300
  }]
}

Input: "محتاج كونكور 5 ضروري"
Output:
{
  "items": [{
    "type": "REQUEST",
    "medication": "Concor 5",
    "medication_raw": "كونكور 5",
    "ai_confidence": 0.9,
    "urgent": true
  }]
}

Input: "اوزمبك واحد ونص"
Output:
{
  "items": [
    {"type": "REQUEST", "medication": "Ozempic 1", "medication_raw": "اوزمبك واحد", "ai_confidence": 0.9},
    {"type": "REQUEST", "medication": "Ozempic 0.5", "medication_raw": "اوزمبك نص", "ai_confidence": 0.9}
  ]
}

`

// FormatMappings creates medication translation map for prompt
func FormatMappings(mappings []*domain.MedicationMapping) string {
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
