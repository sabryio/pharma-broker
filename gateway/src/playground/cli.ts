import { getLegacyMessages, benchmarkMessage } from "./core";
import Table from "cli-table3";
import pLimit from "p-limit";

async function main() {
  const limitArg = parseInt(process.argv[2] || "5");
  const concurrency = parseInt(process.argv[3] || "3"); // Default 3 concurrent messages

  console.log(`🚀 AI Playground CLI`);
  console.log(`   - Messages: ${limitArg}`);
  console.log(`   - Concurrency: ${concurrency}`);

  const limit = pLimit(concurrency);

  try {
    const messages = await getLegacyMessages(limitArg);

    if (messages.length === 0) {
      console.error("❌ No messages found in database.");
      process.exit(1);
    }

    console.log(`\n⏳ Benchmarking with Structured Output...`);

    // Process in parallel
    const tasks = messages.map((msg: any, index: number) => {
      return limit(async () => {
        const comparisons = await benchmarkMessage(msg.content);
        return {
          index,
          msg,
          comparisons,
        };
      });
    });

    const results = await Promise.all(tasks);

    // Sort by index to maintain order in output
    results.sort((a, b) => a.index - b.index);

    // Render Output
    for (const { index, msg, comparisons } of results) {
      console.log(`\n========================================`);
      console.log(`MESSAGE ${index + 1}`);
      console.log(
        `"${msg.content.replace(/\n/g, " ").substring(0, 80)}${
          msg.content.length > 80 ? "..." : ""
        }"`
      );
      console.log(`========================================`);

      const table = new Table({
        head: [
          "Model",
          "Score",
          "Rel",
          "Items",
          "Latency",
          "Issues",
          "Content",
        ],
        colWidths: [12, 8, 8, 8, 10, 20, 50],
        wordWrap: true,
      });

      for (const [model, result] of Object.entries(comparisons) as [
        string,
        any
      ][]) {
        if (result.error) {
          table.push([model, "ERR", "❌", "-", "-", result.error, "-"]);
        } else {
          const q = result.quality;
          const completeness = `${q.completeness_score}%`;

          let relIcon = "❓";
          if (q.overall_reliability === "HIGH") relIcon = "✅";
          else if (q.overall_reliability === "MEDIUM") relIcon = "⚠️";
          else relIcon = "❌";

          // Format Issues
          const issues = [];
          if (q.warnings.length > 0) issues.push(...q.warnings);
          q.item_scores.forEach((s: any) => {
            if (s.is_hallucination)
              issues.push(`HALLUCINATION: ${s.hallucination_reason}`);
          });
          const issuesStr = issues.length > 0 ? issues.join(", ") : "None";

          // Format Content (First 2 items summary)
          const itemsStr =
            result.parsed.items
              .map(
                (i: any) =>
                  `${i.type === "OFFER" ? "🟢" : "🔵"} ${i.medication} (${
                    i.quantity || 0
                  })`
              )
              .join("\n") || "No items";

          table.push([
            model,
            completeness,
            relIcon,
            result.item_count,
            `${result.latency}ms`,
            issuesStr,
            itemsStr,
          ]);
        }
      }

      console.log(table.toString());
    }

    console.log(`\n✅ Benchmark complete.`);
  } catch (err) {
    console.error("❌ Fatal error:", err);
    process.exit(1);
  }
}

main();
