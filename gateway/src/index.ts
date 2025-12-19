/**
 * PharmaBroker AI Gateway
 *
 * TypeScript service that handles:
 * - AI SDK integration (Vercel AI SDK)
 * - Dashboard API proxy to Rust core
 * - WebSocket for real-time updates
 */

import { Hono } from "hono";
import { cors } from "hono/cors";
import { logger } from "hono/logger";

const app = new Hono();

// Middleware
app.use("*", logger());
app.use("*", cors());

// Configuration
const CORE_API_URL = process.env.CORE_API_URL || "http://localhost:8080";

// Health check
app.get("/health", (c) => {
  return c.json({
    status: "healthy",
    service: "pharma-gateway",
    version: "0.1.0",
    timestamp: new Date().toISOString(),
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

// AI parsing endpoint
app.post("/ai/parse", async (c) => {
  const body = await c.req.json();
  const { content } = body;

  if (!content) {
    return c.json({ error: "Content is required" }, 400);
  }

  // TODO: Implement AI SDK parsing
  // const result = await streamText({
  //   model: openai('gpt-4-turbo'),
  //   system: PHARMA_PROMPT,
  //   prompt: content,
  // })

  return c.json({
    message: "AI parsing endpoint - TODO: Implement with AI SDK",
    content_length: content.length,
  });
});

// Start server
const port = parseInt(process.env.PORT || "3000");
console.log(`🌐 PharmaBroker AI Gateway starting on port ${port}...`);

export default {
  port,
  fetch: app.fetch,
};
