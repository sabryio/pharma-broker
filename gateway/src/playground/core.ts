import { SQL } from "bun";
import { generateText } from "ai";
import {
  models,
  SYSTEM_PROMPT,
  ParseResultSchema,
  type ParseResult,
} from "../shared";
import { evaluateQuality } from "./quality";

const DB_URL =
  process.env.DATABASE_URL ||
  "postgres://postgres:password@localhost:5432/pharmabroker?sslmode=disable";
const db = new SQL(DB_URL);

export async function getLegacyMessages(limit: number) {
  return await db`SELECT content, sender_name, group_name FROM raw_messages LIMIT ${limit}`;
}

function extractJson(text: string): string {
  // Try to extract JSON from markdown code blocks or raw text
  const codeBlockMatch = text.match(/```(?:json)?\s*([\s\S]*?)```/);
  if (codeBlockMatch) return codeBlockMatch[1].trim();

  // Try to find JSON object directly
  const jsonMatch = text.match(/\{[\s\S]*\}/);
  if (jsonMatch) return jsonMatch[0];

  return text;
}

export async function benchmarkMessage(content: string) {
  const modelEntries = Object.entries(models);

  // Run models in parallel for this message
  const results = await Promise.all(
    modelEntries.map(async ([name, modelInstance]) => {
      const start = performance.now();
      try {
        const { text } = await generateText({
          model: modelInstance,
          system: SYSTEM_PROMPT,
          prompt: `Parse this message and respond with JSON only: "${content}"`,
          temperature: 0.1,
        });

        // Parse and validate the response
        const jsonStr = extractJson(text);
        const parsed = JSON.parse(jsonStr) as ParseResult;
        const validated = ParseResultSchema.parse(parsed);

        // Evaluate Quality
        const quality = evaluateQuality(validated.items || [], content);

        return [
          name,
          {
            latency: Math.round(performance.now() - start),
            item_count: validated.items.length,
            quality,
            parsed: validated,
            raw: text,
          },
        ];
      } catch (err) {
        return [name, { error: String(err) }];
      }
    })
  );

  return Object.fromEntries(results);
}
