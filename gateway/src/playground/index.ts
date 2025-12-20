import { Hono } from "hono";
import { getLegacyMessages, benchmarkMessage } from "./core";

const playground = new Hono();

playground.get("/benchmark", async (c) => {
  const limit = parseInt(c.req.query("limit") || "5");

  try {
    const messages = await getLegacyMessages(limit);
    if (messages.length === 0) return c.json({ error: "No messages" }, 404);

    const results = [];
    for (const msg of messages) {
      const comparisons = await benchmarkMessage(msg.content);
      results.push({
        content: msg.content,
        sender: msg.sender_name,
        comparisons,
      });
    }

    return c.json({
      success: true,
      total_messages: messages.length,
      results,
    });
  } catch (err) {
    return c.json({ error: String(err) }, 500);
  }
});

export default playground;
