/**
 * PharmaBroker AI Gateway
 *
 * TypeScript service that handles:
 * - AI SDK integration (Vercel AI SDK with OpenAI-compatible local LLM)
 * - Dashboard API proxy to Rust core
 * - Medication message parsing (ported from legacy Go prompts)
 */

import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger } from "hono/logger";
import { generateText, streamText } from "ai";
import { createOpenAICompatible } from "@ai-sdk/openai-compatible";
import { z } from "zod";

const app = new Hono();

// Middleware
app.use("*", logger());
app.use("*", cors());

// Configuration
const CORE_API_URL = process.env.CORE_API_URL || "http://localhost:8080";
const AI_BASE_URL =
  process.env.AI_BASE_URL || "http://localhost:12434/engines/llama.cpp/v1";
const AI_MODEL_ID = process.env.AI_MODEL_ID || "ai/qwen3-vl:latest";

// Create OpenAI-compatible provider for local LLM (Docker Model Runner)
const provider = createOpenAICompatible({
  name: "docker-model-runner",
  apiKey: process.env.AI_API_KEY || "not-needed",
  baseURL: AI_BASE_URL,
});

const model = provider(AI_MODEL_ID);

// ============================================================================
// PROMPT TEMPLATES (Ported from legacy/ai/prompts/templates.go)
// ============================================================================

const SYSTEM_PROMPT = `# Role
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
- Detect Arabic words: "نص" (0.5), "ربع" (0.25), "تلاتة" (3), "علبتين" (2).
- Number words: "واحد" (1), "اتنين" (2), "تلات" (3), "اربع" (4), "خمس" (5).
- Units: "علبة" (box), "شريط" (strip), "امبول" (ampoule), "ق" (piece).
- CRITICAL: If quantity is NOT explicitly stated, set quantity: 0 (DO NOT guess).

## 3a. Price Extraction (VERY IMPORTANT)
- Arabic price patterns: "ب 300", "بـ٣٠٠", "ب٣٠٠", "300 جنيه", "300 ج", "السعر 300"
- English price patterns: "300 EGP", "for 300", "price: 300", "@ 300"
- CRITICAL: If price is NOT explicitly stated, set price: 0 (DO NOT guess).
- For REQUESTs: max_price = 0 unless explicitly stated with "أقصى", "max", "حد أقصى".
- Common price keywords: "ب", "بسعر", "السعر", "الواحدة ب", "للعلبة"

## 4. Confidence Scoring (Confidence-Weighted)
- 1.0: Exact map match + clear price/qty.
- 0.8: Clear intent + recognizable medication name.
- 0.5: Ambiguous name or unclear if it's a medication.
- <0.5: Likely noise.

## 5. Exclusions (Negative Constraints)
- IGNORE: "تواصل", "استفسار", "خاص", "موبيل", "010xxxx", "011xxxx".
- IGNORE: "سعر", "بكام" (Price inquiries are NOT Requests unless explicit "Need").

# Medication Mappings (Use these for exact translation)
{{MEDICATION_MAPPINGS}}

# Context & Replies
- If "Replying to" is present, inherit context (Medication name, Intent).
- "نفسه" or "منه" refers to the medication in the replied message.
- "بكام؟" on an OFFER -> Contextual Query (ignore as Request, unless "عايز منه").
- IMPORTANT: Use the provided Medication Mappings to resolve Arabic names to English brand names.

# Examples (Few-Shot)

## ✅ Arabic Example
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

## ✅ English Example
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

## ❌ Bad Example (Avoid)
Input: "للتواصل 01012345678"
CORRECT Output: {"items": []}
(Phone numbers are NOT medications)`;

// Zod schema for parsed medication data
const MedicationItemSchema = z.object({
  type: z.enum(["OFFER", "REQUEST"]),
  medication: z.string(),
  medication_raw: z.string(),
  ai_confidence: z.number().min(0).max(1),
  quantity: z.number().default(0),
  unit: z.string().nullable().optional(),
  price: z.number().default(0),
  max_price: z.number().optional(),
  urgent: z.boolean().optional(),
  notes: z.string().optional(),
});

const ParseResultSchema = z.object({
  items: z.array(MedicationItemSchema),
});

type ParseResult = z.infer<typeof ParseResultSchema>;

// Health check
app.get("/health", (c) => {
  return c.json({
    status: "healthy",
    service: "pharma-gateway",
    version: "0.1.0",
    timestamp: new Date().toISOString(),
    ai: {
      provider: "openai-compatible",
      base_url: AI_BASE_URL,
      model: AI_MODEL_ID,
    },
  });
});

// Proxy to Rust core API
app.all("/api/*", async (c) => {
  const path = c.req.path;
  const url = `${CORE_API_URL}${path}`;

  try {
    const response = await fetch(url, {
      method: c.req.method,
      headers: c.req.header(),
      body: c.req.method !== "GET" ? await c.req.text() : undefined,
    });

    const data = await response.json();
    return c.json(data, response.status as any);
  } catch (error) {
    console.error("Proxy error:", error);
    return c.json({ error: "Core service unavailable" }, 503);
  }
});

// AI parsing endpoint (main endpoint - ported from Go)
app.post("/ai/parse", async (c) => {
  const body = await c.req.json();
  const { content, sender_name, group_name, reply_to } = body;

  if (!content) {
    return c.json({ error: "Content is required" }, 400);
  }

  // Build prompt similar to Go's BuildParsePrompt
  let prompt = "=== MESSAGE TO PARSE ===\n\n";
  if (sender_name) prompt += `From: ${sender_name}\n`;
  if (group_name) prompt += `Group: ${group_name}\n`;
  if (reply_to) prompt += `Replying to: "${reply_to.substring(0, 200)}"\n`;
  prompt += `Content:\n${content}\n\n`;
  prompt += "=== END MESSAGE ===\n\nReturn valid JSON only.\n";

  let systemPrompt = SYSTEM_PROMPT;
  if (
    Array.isArray(body.medication_mappings) &&
    body.medication_mappings.length > 0
  ) {
    const mappingList = body.medication_mappings
      .map((m: string) => `- ${m}`)
      .join("\n");
    systemPrompt = systemPrompt.replace("{{MEDICATION_MAPPINGS}}", mappingList);
  } else {
    systemPrompt = systemPrompt.replace(
      "{{MEDICATION_MAPPINGS}}",
      "No mappings available."
    );
  }

  try {
    const { text, usage } = await generateText({
      model,
      system: systemPrompt,
      prompt,
      maxOutputTokens: 1000,
      temperature: 0.3,
    });

    // Extract JSON from response
    let parsed: ParseResult = { items: [] };
    try {
      const jsonMatch = text.match(/```(?:json)?\s*([\s\S]*?)```/) || [
        null,
        text,
      ];
      const jsonStr = jsonMatch[1]?.trim() || text.trim();

      // Find the JSON object in the response
      const jsonStart = jsonStr.indexOf("{");
      const jsonEnd = jsonStr.lastIndexOf("}") + 1;
      if (jsonStart >= 0 && jsonEnd > jsonStart) {
        parsed = JSON.parse(jsonStr.substring(jsonStart, jsonEnd));
      }
    } catch (e) {
      console.error("JSON parsing error:", e);
    }

    return c.json({
      success: true,
      parsed,
      raw_response: text,
      usage,
    });
  } catch (error) {
    console.error("AI parsing error:", error);
    return c.json(
      {
        error: "AI parsing failed",
        message: error instanceof Error ? error.message : "Unknown error",
      },
      500
    );
  }
});

// Streaming AI parsing endpoint
app.post("/ai/parse/stream", async (c) => {
  const body = await c.req.json();
  const { content } = body;

  if (!content) {
    return c.json({ error: "Content is required" }, 400);
  }

  try {
    const result = streamText({
      model,
      system: SYSTEM_PROMPT,
      prompt: `Parse this message:\n\n${content}\n\nReturn valid JSON only.`,
      maxOutputTokens: 1000,
    });

    return result.toTextStreamResponse();
  } catch (error) {
    console.error("AI stream error:", error);
    return c.json({ error: "AI streaming failed" }, 500);
  }
});

// Batch parsing endpoint
app.post("/ai/parse/batch", async (c) => {
  const body = await c.req.json();
  const { messages } = body;

  if (!Array.isArray(messages) || messages.length === 0) {
    return c.json({ error: "messages array is required" }, 400);
  }

  // Build batch prompt
  let prompt = "=== MESSAGES TO PARSE ===\n\n";
  messages.slice(0, 10).forEach((msg: any, i: number) => {
    prompt += `--- Message ${i} ---\n`;
    if (typeof msg === "string") {
      prompt += `Content:\n${msg}\n\n`;
    } else {
      if (msg.sender_name) prompt += `From: ${msg.sender_name}\n`;
      if (msg.group_name) prompt += `Group: ${msg.group_name}\n`;
      prompt += `Content:\n${msg.content}\n\n`;
    }
  });
  prompt += "=== END MESSAGES ===\n\nReturn valid JSON only.\n";

  try {
    const { text, usage } = await generateText({
      model,
      system: SYSTEM_PROMPT,
      prompt,
      maxOutputTokens: 3000,
      temperature: 0.3,
    });

    // Extract JSON
    let parsed: ParseResult = { items: [] };
    try {
      const jsonMatch = text.match(/```(?:json)?\s*([\s\S]*?)```/) || [
        null,
        text,
      ];
      const jsonStr = jsonMatch[1]?.trim() || text.trim();
      const jsonStart = jsonStr.indexOf("{");
      const jsonEnd = jsonStr.lastIndexOf("}") + 1;
      if (jsonStart >= 0 && jsonEnd > jsonStart) {
        parsed = JSON.parse(jsonStr.substring(jsonStart, jsonEnd));
      }
    } catch (e) {
      console.error("JSON parsing error:", e);
    }

    return c.json({
      success: true,
      count: messages.length,
      parsed,
      usage,
    });
  } catch (error) {
    console.error("AI batch error:", error);
    return c.json({ error: "AI batch parsing failed" }, 500);
  }
});

// Simple test endpoint
app.post("/ai/test", async (c) => {
  const body = await c.req.json();
  const { prompt = "Hello, how are you?" } = body;

  try {
    const { text, usage } = await generateText({
      model,
      prompt,
      maxOutputTokens: 100,
      temperature: 0.7,
    });

    return c.json({
      success: true,
      response: text,
      usage,
    });
  } catch (error) {
    console.error("AI test error:", error);
    return c.json(
      {
        error: "AI test failed",
        message: error instanceof Error ? error.message : "Unknown error",
      },
      500
    );
  }
});

// ============================================================================
// EMBEDDING ENDPOINT
// ============================================================================

// Embedding model configuration
const EMBEDDING_BASE_URL = process.env.EMBEDDING_BASE_URL || AI_BASE_URL;
const EMBEDDING_MODEL_ID =
  process.env.EMBEDDING_MODEL_ID || "ai/embeddinggemma:latest";

// Embedding endpoint - generates vector embeddings for medication names
app.post("/ai/embed", async (c) => {
  const body = await c.req.json();
  const { text, texts } = body;

  // Support single text or batch
  const inputTexts: string[] = texts || (text ? [text] : []);

  if (inputTexts.length === 0) {
    return c.json({ error: "text or texts array is required" }, 400);
  }

  try {
    // Call OpenAI-compatible embedding API
    const response = await fetch(`${EMBEDDING_BASE_URL}/embeddings`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${process.env.AI_API_KEY || "not-needed"}`,
      },
      body: JSON.stringify({
        model: EMBEDDING_MODEL_ID,
        input: inputTexts,
      }),
    });

    if (!response.ok) {
      const errorText = await response.text();
      console.error("Embedding API error:", response.status, errorText);
      return c.json(
        { error: "Embedding API failed", status: response.status },
        500
      );
    }

    const result = await response.json();

    // Extract embeddings from response
    const embeddings = result.data?.map((item: any) => item.embedding) || [];

    return c.json({
      success: true,
      embeddings,
      model: EMBEDDING_MODEL_ID,
      dimensions: embeddings[0]?.length || 0,
    });
  } catch (error) {
    console.error("Embedding error:", error);
    return c.json(
      {
        error: "Embedding generation failed",
        message: error instanceof Error ? error.message : "Unknown error",
      },
      500
    );
  }
});

// Start server
const port = parseInt(process.env.PORT || "3000");
console.log(`🌐 PharmaBroker AI Gateway starting on port ${port}...`);
console.log(`📡 Proxying API requests to: ${CORE_API_URL}`);
console.log(`🤖 AI Provider: ${AI_BASE_URL}`);
console.log(`🧠 AI Model: ${AI_MODEL_ID}`);

export default {
  port,
  fetch: app.fetch,
};
