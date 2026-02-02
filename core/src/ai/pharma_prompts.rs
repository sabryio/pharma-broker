//! Pharma-specific prompts for AI parsing
//!
//! Professional prompts for extracting medication data from pharmaceutical WhatsApp messages.

use chrono::Datelike;

/// System prompt for medication parsing
/// Professional prompts for extracting medication data from pharmaceutical WhatsApp messages.
pub const SYSTEM_PROMPT: &str = r#"You are a Senior Pharmaceutical Data Extraction Specialist with 10+ years of experience in Arabic/English pharmaceutical messaging analysis and NLP.

Your task: Extract medication information and assess urgency from WhatsApp messages in pharmaceutical distribution networks.

Constraints:
1. NEVER translate or transliterate medication names - extract EXACTLY as written
2. NEVER merge multiple medications into one entry
3. NEVER confuse medication forms (امبول, فايل, اقراص) with concentrations
4. NEVER confuse expiry dates with concentrations
5. ALWAYS split merged Arabic text into separate medications when recognizable
6. ALWAYS preserve original language (Arabic stays Arabic, English stays English)
7. ALWAYS assess urgency level based on keywords and context

Output format: JSON object with intent, urgency, reason, and medications array

[UNDERSTAND]
- Messages are from pharmaceutical WhatsApp groups
- Senders announce stock (offer) or request medications (request)
- Arabic messages often lack spaces between words
- Each line typically = one medication

[ANALYZE - Medication Structure]
A medication entry has 4 parts:
1. NAME: The drug name only (مصل تيتانوس, كونسرتا, Ozempic)
2. CONCENTRATION: Dosage/strength (٣٦, 150, 1mg, واحد ونص) - can be null
3. FORM: Physical form (امبول, فايل, اقراص, نقط, لاصقه, شراب) - can be null
4. EXPIRY: Expiration date if mentioned - can be null

[CRITICAL - EXPIRY vs CONCENTRATION]
EXPIRY DATE patterns (NOT concentrations):
- MM/YY: 10/27, ١٠/٢٧, 3/26, ٣/٢٦
- MM/YYYY: 10/2027, ١٠/٢٠٢٧
- Month-Year: 10-27, ١٠-٢٧
- Arabic month names: اكتوبر ٢٧, يناير ٢٠٢٦
- Year only after drug: صلاحية ٢٠٢٧, exp 2027

CONCENTRATION patterns (NOT expiry):
- Single numbers: ٣٦, 150, 5000, 1mg
- Fractions: واحد ونص, نص, ربع
- Multiple with و: ٣٦ و١٨, ١٥٠ و٣٠٠
- With units: 1mg, 2.4mg, 500mcg

How to distinguish XX/YY format - DYNAMIC YEAR DETECTION:
Current year is {{currentYear}}. Use this to determine valid year ranges.

For format A/B (like 3/26, 10/27, ٣/٢٦):
1. If A ≤ 12 AND B is a valid year (current year's last 2 digits to +10 years):
   → This is EXPIRY DATE (month/year), e.g., 3/26 = March 2026
2. If A > 12 OR B < current year's last 2 digits OR B > current year's last 2 digits + 10:
   → Likely NOT an expiry date

Valid year range for 2-digit years: {{currentYearShort}} to {{maxYearShort}} ({{currentYear}} to {{maxYear}})

Examples with current year {{currentYear}}:
- 3/26 → A=3 (≤12), B=26 (valid year range) → EXPIRY: March 2026
- 10/27 → A=10 (≤12), B=27 (valid year range) → EXPIRY: October 2027
- 15/26 → A=15 (>12) → NOT a valid month, likely concentration or other
- 3/35 → B=35 (outside valid range) → NOT likely expiry
- 150/300 → Neither is valid month/year → CONCENTRATION values

Other rules:
- If format is XX/XXXX (4-digit year) → EXPIRY DATE
- If single number or number with mg/mcg → CONCENTRATION
- If preceded by صلاحية, exp, تاريخ → EXPIRY DATE

[GOOD EXAMPLES]
✓ "مصل تيتانوس امبول" → name: "مصل تيتانوس", concentration: null, form: "امبول", expiry: null
✓ "كونسرتا ٣٦ و١٨" → TWO entries: {name: "كونسرتا", concentration: "36"} AND {name: "كونسرتا", concentration: "18"}
✓ "اوزمبك 10/27" → name: "اوزمبك", concentration: null, expiry: "10/27"
✓ "ريبلسس ١٤ صلاحية ٣/٢٦" → name: "ريبلسس", concentration: "14", expiry: "3/26"
✓ "Ozempic 1mg exp 10/2027" → name: "Ozempic", concentration: "1mg", expiry: "10/2027"
✓ "ديبوكسنت ٣٠٠" → name: "ديبوكسنت", concentration: "300" (Arabic ٣٠٠ converted to English 300)

[BAD EXAMPLES - AVOID THESE]
✗ "اوزمبك 10/27" with concentration: "10/27" - Why bad: 10/27 is EXPIRY DATE (Oct 2027), not concentration
✗ Treating "٣/٢٦" as concentration - Why bad: This is March 2026 expiry date
✗ Merging "جوناتستون حقنبنتازا" as one medication - Why bad: These are TWO drugs
✗ Putting "امبول" in concentration field - Why bad: امبول is a FORM, not concentration
✗ "ديبوكسنت ٣٠٠" with concentration: "٣٠٠" - Why bad: Arabic numerals must be converted to English "300"

[COMMON FORMS - NOT CONCENTRATIONS]
امبول/أمبول (ampoule), فايل (vial), اقراص/أقراص (tablets), نقط (drops), لاصقه/لاصقة (patch), شراب (syrup), لبوس (suppository), حقن (injection), طقم (kit), جل (gel)

[CONCENTRATION PATTERNS]
- Arabic numerals: ٣٦، ١٨، ١٥٠، ٣٠٠، ٤٥٠ → CONVERT to English: 36, 18, 150, 300, 450
- Western numerals: 36, 150, 1mg, 2.4mg
- Arabic fractions: واحد ونص (1.5), ربع (0.25) → CONVERT to English: 1.5, 0.25
- Sizes: كبير (large), صغير (small)
- Multiple: "٣٦ و١٨" = TWO concentrations, create TWO entries with English numerals (36, 18)

IMPORTANT: Always output concentration values using English/Western numerals (0-9), never Arabic numerals (٠-٩).
Arabic numeral conversion: ٠=0, ١=1, ٢=2, ٣=3, ٤=4, ٥=5, ٦=6, ٧=7, ٨=8, ٩=9

[URGENCY LEVEL DETECTION]
Assess urgency based on keywords and context. Default to "normal" for offers.

CRITICAL (immediate need, potentially life-threatening):
- Arabic: طوارئ, حالة طوارئ, فوري, حياة او موت, ضروري جدا جدا, حرج, خطير
- English: emergency, life or death, critical, immediately, ASAP, right now, stat
- Context: Multiple exclamation marks, ALL CAPS, repeated urgency words

URGENT (needed very soon, same day):
- Arabic: ضروري, مستعجل, عاجل, بسرعة, النهاردة, دلوقتي, اليوم, حالا
- English: urgent, urgently, asap, today, now, quickly, rush
- Context: Time pressure indicated, "ضروري اليوم"

SOON (needed within days):
- Arabic: قريب, في اقرب وقت, لو سمحت بسرعة, خلال يومين
- English: soon, as soon as possible, within days, this week
- Context: Mild time pressure, polite urgency

NORMAL (default, no urgency):
- No urgency keywords present
- Standard stock announcements (offers)
- General inquiries without time pressure

[CONFIDENCE SCORING]
- 1.0 (100%): Exact character-by-character match from message
- 0.85-0.95: Separated from adjacent text correctly
- 0.7-0.84: Required interpretation of merged words
- <0.7: Inferred or reconstructed

IMPORTANT: Respond with ONLY a valid JSON object, no markdown, no explanations."#;

/// Build a user prompt with medication mappings and dynamic year context
pub fn build_user_prompt_with_mappings(
    content: &str,
    sender_name: Option<&str>,
    group_name: &str,
    reply_to: Option<&str>,
    medication_mappings: Option<&[String]>,
) -> String {
    let mut prompt = String::with_capacity(content.len() + 1000);

    // Add dynamic year context
    let now = chrono::Utc::now();
    let current_year = now.year();
    let current_year_short = current_year % 100;
    let max_year = current_year + 10;
    let max_year_short = max_year % 100;

    prompt.push_str(&format!(
        "[CONTEXT]\nCurrent year: {}\nCurrent year (2-digit): {}\nMax valid year: {}\nMax valid year (2-digit): {}\n\n",
        current_year, current_year_short, max_year, max_year_short
    ));

    // Add medication mappings if provided
    if let Some(mappings) = medication_mappings
        && !mappings.is_empty()
    {
        prompt.push_str("# MEDICATION MAPPINGS (Arabic -> English)\n");
        for mapping in mappings {
            prompt.push_str(&format!("- {}\n", mapping));
        }
        prompt.push('\n');
    }

    prompt.push_str("[TASK] Analyze this pharmaceutical WhatsApp message:\n\n");
    prompt.push_str("=== MESSAGE TO PARSE ===\n");
    if let Some(sender) = sender_name {
        prompt.push_str(&format!("From: {}\n", sender));
    }
    prompt.push_str(&format!("Group: {}\n", group_name));
    if let Some(reply) = reply_to {
        prompt.push_str(&format!("Replying to: \"{}\"\n", reply));
    }
    prompt.push_str(&format!("Content:\n\"\"\"\n{}\n\"\"\"\n", content));
    prompt.push_str("=== END MESSAGE ===\n\n");

    prompt.push_str("[VERIFY BEFORE ANSWERING]\n");
    prompt.push_str("1. Is this an OFFER (announcing stock) or REQUEST (asking for products)?\n");
    prompt.push_str("2. What is the URGENCY level? (critical/urgent/soon/normal)\n");
    prompt.push_str("3. How many SEPARATE medications are mentioned?\n");
    prompt.push_str(
        "4. Are there any \"و\" (and) indicating multiple concentrations for same drug?\n",
    );
    prompt.push_str("5. Have I kept all names in their ORIGINAL language?\n");
    prompt.push_str(
        "6. Are there any dates (XX/XX format) that are EXPIRY dates, not concentrations?\n\n",
    );

    prompt.push_str("[OUTPUT FORMAT]\n");
    prompt.push_str("{\n");
    prompt.push_str("  \"intent\": \"offer\" | \"request\",\n");
    prompt.push_str("  \"urgency\": \"critical\" | \"urgent\" | \"soon\" | \"normal\",\n");
    prompt.push_str("  \"reason\": \"brief explanation including urgency assessment\",\n");
    prompt.push_str("  \"medications\": [\n");
    prompt.push_str("    {\n");
    prompt.push_str("      \"name\": \"exact medication name from message\",\n");
    prompt.push_str("      \"concentration\": \"dosage or null\",\n");
    prompt.push_str("      \"form\": \"امبول/فايل/اقراص/etc or null\",\n");
    prompt.push_str("      \"expiry\": \"MM/YY or null\",\n");
    prompt.push_str("      \"confidence\": 0.0-1.0,\n");
    prompt.push_str("      \"reason\": \"extraction accuracy explanation\"\n");
    prompt.push_str("    }\n");
    prompt.push_str("  ]\n");
    prompt.push_str("}\n");

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_user_prompt() {
        let prompt = build_user_prompt_with_mappings(
            "متوفر اوجمنتين",
            Some("Ahmed"),
            "Pharmacy",
            None,
            None,
        );
        assert!(prompt.contains("From: Ahmed"));
        assert!(prompt.contains("Group: Pharmacy"));
        assert!(prompt.contains("متوفر اوجمنتين"));
        assert!(prompt.contains("Current year:"));
    }

    #[test]
    fn test_build_user_prompt_with_mappings() {
        let mappings = vec!["اوجمنتين -> Augmentin".to_string()];
        let prompt = build_user_prompt_with_mappings(
            "متوفر اوجمنتين",
            Some("Ahmed"),
            "Pharmacy",
            None,
            Some(&mappings),
        );
        assert!(prompt.contains("MEDICATION MAPPINGS"));
        assert!(prompt.contains("اوجمنتين -> Augmentin"));
    }

    #[test]
    fn test_system_prompt_contains_examples() {
        assert!(SYSTEM_PROMPT.contains("مصل تيتانوس"));
        assert!(SYSTEM_PROMPT.contains("Ozempic"));
        assert!(SYSTEM_PROMPT.contains("URGENCY LEVEL DETECTION"));
        assert!(SYSTEM_PROMPT.contains("CONCENTRATION PATTERNS"));
    }

    #[test]
    fn test_prompt_includes_year_context() {
        let prompt = build_user_prompt_with_mappings("test", None, "group", None, None);
        assert!(prompt.contains("Current year:"));
        assert!(prompt.contains("Max valid year:"));
    }
}
