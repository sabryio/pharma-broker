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

	// Inject dynamic mappings in GoTOON format (tabular array) for token efficiency
	// Format: mapping[N]{arabic,english}:
	if len(mappings) > 0 {
		sb.WriteString(fmt.Sprintf("\n\nmapping[%d]{arabic,english}:\n", len(mappings)))
		for arabic, english := range mappings {
			// Simple CSV-like lines, no quotes needed unless special chars (rare in these simple names)
			sb.WriteString(fmt.Sprintf("  %s,%s\n", arabic, english))
		}
		sb.WriteString("\n")
		sb.WriteString("Use the mapping table above to translate Arabic medication names to English.\n")
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
	sb.WriteString(parsingExamples)

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
- **CRITICAL: Translation Map Priority**:
  - Check the "KNOWN MEDICATION TRANSLATIONS" list below.
  - If a medication name **CONTAINS** a key from the map, replace that part with the English name.
  - Example: Input "بنتازا أقراص" -> Map has "بنتازا":"Pentasa" -> Output "Pentasa Tablets" (or "Pentasa أقراص").
  - Do NOT leave the brand name in Arabic if it exists in the map.

- **CRITICAL: Translate/Transliterate the brand name EXACTLY as written.**
- **CRITICAL: IF THE NAME IS IN ARABIC AND NOT IN THE MAP, TRANSLITERATE IT.**
- Example: "Suvreza" -> "سوفريزا"
- Example: "Mounjaro" -> "Mounjaro" (NOT "Tirzepatide")
- Example: "Panadol" -> "Panadol"
- Normalize common misspellings
- Include dosage forms: tablets, capsules, ampules, syrup, etc.
- Include strength when mentioned: 500mg, 1g, etc.
- **Exclusions**: Do NOT extract communication terms as items:
  - "واتس", "فون", "تواصل", "خاص", "موبيل"
  - "WhatsApp", "Phone", "DM", "Inbox", "Contact"

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

const parsingExamples = `
## Examples

Input message: "عندي اوجمنتين 1 جم ٥ علب ب ٣٠٠ جنيه الواحدة"
Output:
{
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
}`
