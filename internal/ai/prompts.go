package ai

import (
	"fmt"
	"strings"

	"pharmabroker/internal/domain"
)

// buildParsePrompt creates the prompt for parsing pharmaceutical messages
func buildParsePrompt(messages []*domain.RawMessage, mappings map[string]string) string {
	var sb strings.Builder

	sb.WriteString(systemPrompt)

	// Inject dynamic mappings if available
	if len(mappings) > 0 {
		sb.WriteString("\n\n### KNOWN MEDICATION TRANSLATIONS (Priority)\n")
		sb.WriteString("Use these specific translations if encountered:\n")
		for arabic, english := range mappings {
			sb.WriteString(fmt.Sprintf("- %s: %s\n", arabic, english))
		}
	}

	sb.WriteString("\n\n")
	sb.WriteString("=== MESSAGES TO PARSE ===\n\n")

	for i, msg := range messages {
		sb.WriteString(fmt.Sprintf("--- Message %d ---\n", i))
		sb.WriteString(fmt.Sprintf("From: %s\n", msg.SenderName))
		sb.WriteString(fmt.Sprintf("Group: %s\n", msg.GroupName))
		sb.WriteString(fmt.Sprintf("Content:\n%s\n\n", msg.Content))
	}

	sb.WriteString("=== END MESSAGES ===\n\n")
	sb.WriteString(responseFormat)

	return sb.String()
}

const systemPrompt = `أنت خبير في تحليل رسائل سوق الأدوية المصرية على واتساب.
مهمتك استخراج عروض الأدوية (OFFER) وطلبات الأدوية (REQUEST) من النصوص العربية غير الرسمية.

You are an expert parser for Egyptian pharmaceutical WhatsApp messages.
Your task is to extract medication OFFERS and REQUESTS from informal Arabic text.

## Important Rules:

### Language Handling
- Messages are in Egyptian Arabic dialect (عامية مصرية)
- Common offer phrases: عندي، متوفر، للبيع، available، متاح، موجود
- Common request phrases: محتاج، عايز، مطلوب، wanted، need، محتاجين
- Handle mixed Arabic/English text
- Handle transliterated drug names (e.g., "أوجمنتين" = Augmentin)

### Medication Extraction
- Extract brand names and generic names
- **CRITICAL: Do NOT guess generic names or active ingredients.**
- **CRITICAL: Translate/Transliterate the brand name EXACTLY as written.**
- Example: "Monjaro" -> "Monjaro" (NOT "Tirzepatide", NOT "Moxifloxacin")
- Example: "Panadol" -> "Panadol" (NOT "Paracetamol")
- Normalize common misspellings
- Include dosage forms: tablets, capsules, ampules, syrup, etc.
- Include strength when mentioned: 500mg, 1g, etc.

### Quantity & Price
- Extract quantities with units (علبة, شريط, امبول, boxes, strips)
- Prices in EGP (Egyptian Pounds) - symbols: ج.م، جنيه، EGP, LE
- Handle ranges (e.g., "من 50 ل 100")

### Message Classification
- OFFER: Seller has medication available
- REQUEST: Buyer needs medication
- BOTH: Message contains both offers and requests (common in "swap" messages)
- **If the message is just a list of medications without clear context, assume it is an OFFER.**
- If message is general chat or unclear, return empty items array

### Urgency Detection
- Mark as urgent if contains: ضروري، عاجل، urgent، ASAP، مستعجل`

const responseFormat = `## Response Format

Return a JSON array with one object per message. Each object has:
- message_index: 0-based index of the message
- items: array of extracted items
- error: optional error message if parsing failed

Each item in items array:
{
  "type": "OFFER" | "REQUEST" | "BOTH",
  "medication": "normalized drug name in English",
  "medication_raw": "original text as written",
  "quantity": number or 0 if not specified,
  "unit": "boxes" | "strips" | "ampules" | "bottles" | null,
  "price": number or 0 if not specified (for offers),
  "max_price": number or 0 if not specified (for requests),
  "urgent": true/false (for requests),
  "notes": "any additional details (expiry, batch, currency, etc)"
}

## Examples

Input message: "عندي اوجمنتين 1 جم ٥ علب ب ٣٠٠ جنيه الواحدة"
Output:
{
  "message_index": 0,
  "items": [
    {
      "type": "OFFER",
      "medication": "Augmentin 1g",
      "medication_raw": "اوجمنتين 1 جم",
      "quantity": 5,
      "unit": "boxes",
      "price": 300,
      "notes": "Currency: EGP"
    }
  ]
}

Input message: "محتاج كونكور 5 ضروري"
Output:
{
  "message_index": 0,
  "items": [
    {
      "type": "REQUEST",
      "medication": "Concor 5mg",
      "medication_raw": "كونكور 5",
      "quantity": 0,
      "unit": null,
      "max_price": 0,
      "urgent": true,
      "notes": ""
    }
  ]
}

Now parse the following messages and return ONLY valid JSON:`
