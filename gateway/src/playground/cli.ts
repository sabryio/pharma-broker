import { getLegacyMessages, benchmarkMessage } from "./core";

async function main() {
  const limit = parseInt(process.argv[2] || "5");
  console.log(`🚀 AI Playground CLI - Benchmarking ${limit} messages...\n`);

  try {
    const messages = await getLegacyMessages(limit);

    if (messages.length === 0) {
      console.error("❌ No messages found in database.");
      process.exit(1);
    }

    for (let i = 0; i < messages.length; i++) {
      const msg = messages[i];
      console.log(`--- [Message ${i + 1}] ---`);
      console.log(`Content: ${msg.content}\n`);

      const comparisons = await benchmarkMessage(msg.content);

      for (const [model, result] of Object.entries(comparisons)) {
        if (result.error) {
          console.log(`  🤖 ${model}: ❌ Error: ${result.error}`);
        } else {
          console.log(`  🤖 ${model}:`);
          console.log(`     ⏱️ Latency: ${result.latency}ms`);
          console.log(`     📦 Items: ${result.item_count}`);
          console.log(
            `     📄 Result: ${JSON.stringify(result.parsed.items)}\n`
          );
        }
      }
      console.log("---------------------------\n");
    }

    console.log("✅ Benchmark complete.");
  } catch (err) {
    console.error("❌ Fatal error:", err);
    process.exit(1);
  }
}

main();
