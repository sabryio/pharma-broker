import { SQL } from "bun";
import { generateText } from "ai";
import { models, SYSTEM_PROMPT } from "../shared";

const DB_URL =
  process.env.DATABASE_URL ||
  "postgres://pharma:pharma@localhost:5432/pharmabroker";
const db = new SQL(DB_URL);

export async function getLegacyMessages(limit: number) {
  return await db`SELECT content, sender_name, group_name FROM raw_messages LIMIT ${limit}`;
}

export async function benchmarkMessage(content: string) {
  const comparisons: Record<string, any> = {};

  for (const [name, modelInstance] of Object.entries(models)) {
    const start = performance.now();
    try {
      const { text } = await generateText({
        model: modelInstance as any,
        system: SYSTEM_PROMPT,
        prompt: `Parse this message: "${content}"`,
        temperature: 0.1,
        maxOutputTokens: 1000,
      });

      let parsed = { items: [] };
      try {
        const startIdx = text.indexOf("{");
        const endIdx = text.lastIndexOf("}") + 1;
        if (startIdx >= 0 && endIdx > startIdx) {
          parsed = JSON.parse(text.substring(startIdx, endIdx));
        }
      } catch (e) {}

      comparisons[name] = {
        latency: Math.round(performance.now() - start),
        item_count: Array.isArray(parsed.items) ? parsed.items.length : 0,
        parsed,
        raw: text.substring(0, 100) + "...",
      };
    } catch (err) {
      comparisons[name] = { error: String(err) };
    }
  }

  return comparisons;
}
