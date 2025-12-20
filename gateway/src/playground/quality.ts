/**
 * AI Parsing Quality Metrics
 * Ported and enhanced from analysis/scripts/07_ai_parsing_quality.py
 */

export interface QualityResult {
  completeness_score: number; // 0-100
  item_scores: {
    has_medication: boolean;
    has_quantity: boolean;
    has_price_or_max: boolean;
    has_unit: boolean;
    is_hallucination: boolean;
    hallucination_reason?: string;
  }[];
  overall_reliability: "HIGH" | "MEDIUM" | "LOW";
  warnings: string[];
}

const HALLUCINATION_KEYWORDS = [
  "whatsapp",
  "واتساب",
  "تواصل",
  "رقم",
  "010",
  "011",
  "012",
  "015",
  "اتصال",
  "الخاص",
  "dm",
  "phone",
  "mobile",
  "سعر",
  "بكام",
];

const INTENT_KEYWORDS = {
  REQUEST: [
    "محتاج",
    "مطلوب",
    "عايز",
    "نقص",
    "لو حد عنده",
    "مين عنده",
    "ابغى",
    "wanted",
    "need",
    "searching",
  ],
  OFFER: [
    "عندي",
    "متوفر",
    "موجود",
    "للبيع",
    "عندنا",
    "يوجد",
    "available",
    "selling",
    "stock",
  ],
};

export function evaluateQuality(
  parsedItems: any[],
  rawMessage: string
): QualityResult {
  const warnings: string[] = [];
  const itemScores = parsedItems.map((item) => {
    const med = String(item.medication || "").toLowerCase();
    const raw = String(item.medication_raw || "").toLowerCase();

    // Hallucination Check
    let isHallucination = false;
    let hallucinationReason = "";

    for (const kw of HALLUCINATION_KEYWORDS) {
      if (med.includes(kw) || raw.includes(kw)) {
        isHallucination = true;
        hallucinationReason = `Contains noise keyword: ${kw}`;
        break;
      }
    }

    if (med.length < 3 && med.length > 0) {
      isHallucination = true;
      hallucinationReason = "Medication name too short";
    }

    return {
      has_medication:
        !!item.medication && item.medication !== "No mappings available.",
      has_quantity: (Number(item.quantity) || 0) > 0,
      has_price_or_max: (Number(item.price) || Number(item.max_price) || 0) > 0,
      has_unit: !!item.unit,
      is_hallucination: isHallucination,
      hallucination_reason: hallucinationReason,
    };
  });

  // Calculate Completeness (Ported logic from Python)
  let totalPoints = 0;
  let earnedPoints = 0;

  itemScores.forEach((score) => {
    totalPoints += 4; // med, qty, price, unit
    if (score.has_medication) earnedPoints += 1;
    if (score.has_quantity) earnedPoints += 1;
    if (score.has_price_or_max) earnedPoints += 1;
    if (score.has_unit) earnedPoints += 1;

    if (score.is_hallucination) earnedPoints = Math.max(0, earnedPoints - 2); // Penalty
  });

  const completenessScore =
    totalPoints > 0 ? (earnedPoints / totalPoints) * 100 : 0;

  // Reliability Logic
  let reliability: "HIGH" | "MEDIUM" | "LOW" = "HIGH";
  if (completenessScore < 50) reliability = "LOW";
  else if (completenessScore < 80 || itemScores.some((s) => s.is_hallucination))
    reliability = "MEDIUM";

  // Intent Validation
  const hasRequestKeywords = INTENT_KEYWORDS.REQUEST.some((kw) =>
    rawMessage.includes(kw)
  );
  const hasOfferKeywords = INTENT_KEYWORDS.OFFER.some((kw) =>
    rawMessage.includes(kw)
  );

  const extractedTypes = new Set(parsedItems.map((i) => i.type));
  if (hasRequestKeywords && !extractedTypes.has("REQUEST")) {
    warnings.push(
      "Message contains REQUEST keywords but AI didn't extract any REQUEST items."
    );
  }
  if (hasOfferKeywords && !extractedTypes.has("OFFER") && !hasRequestKeywords) {
    warnings.push(
      "Message looks like an OFFER but AI didn't extract any OFFER items."
    );
  }

  return {
    completeness_score: Math.round(completenessScore),
    item_scores: itemScores,
    overall_reliability: reliability,
    warnings,
  };
}
