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
import { z } from "zod";
import playground from "./playground/index";
import { SYSTEM_PROMPT, defaultModel, AI_BASE_URL } from "./shared";

const app = new Hono();

// Middleware
app.use("*", logger());
app.use("*", cors());

// Mount Playground
app.route("/playground", playground);

// Configuration
const CORE_API_URL = process.env.CORE_API_URL || "http://localhost:8080";

const model = defaultModel;
const AI_MODEL_ID = "ai/qwen3-vl:latest"; // Used for health check display

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

// Single embedding endpoint - for one text
app.post("/ai/embed", async (c) => {
  const body = await c.req.json();
  const { text } = body;

  if (!text || typeof text !== "string") {
    return c.json({ error: "text (string) is required" }, 400);
  }

  try {
    const response = await fetch(`${EMBEDDING_BASE_URL}/embeddings`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${process.env.AI_API_KEY || "not-needed"}`,
      },
      body: JSON.stringify({
        model: EMBEDDING_MODEL_ID,
        input: [text],
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
    const embedding = result.data?.[0]?.embedding || [];

    return c.json({
      success: true,
      embeddings: [embedding],
      model: EMBEDDING_MODEL_ID,
      dimensions: embedding.length,
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

// Batch embedding endpoint - for multiple texts in one call
app.post("/ai/embed/batch", async (c) => {
  const body = await c.req.json();
  const { texts } = body;

  if (!Array.isArray(texts) || texts.length === 0) {
    return c.json({ error: "texts (string[]) is required" }, 400);
  }

  try {
    const response = await fetch(`${EMBEDDING_BASE_URL}/embeddings`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${process.env.AI_API_KEY || "not-needed"}`,
      },
      body: JSON.stringify({
        model: EMBEDDING_MODEL_ID,
        input: texts,
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
    const embeddings = result.data?.map((item: any) => item.embedding) || [];

    return c.json({
      success: true,
      embeddings,
      model: EMBEDDING_MODEL_ID,
      dimensions: embeddings[0]?.length || 0,
    });
  } catch (error) {
    console.error("Batch embedding error:", error);
    return c.json(
      {
        error: "Batch embedding generation failed",
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
