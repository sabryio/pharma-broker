import { query, mutation } from "./_generated/server";
import { v } from "convex/values";

// Save feedback record (matches entity/feedback_record.rs)
export const save = mutation({
  args: {
    matchId: v.string(),
    userId: v.string(),
    confirmed: v.boolean(),
    medicationScore: v.number(),
    dosageScore: v.number(),
    quantityScore: v.number(),
    priceScore: v.number(),
    recencyScore: v.number(),
    totalScore: v.number(),
  },
  handler: async (ctx, args) => {
    return await ctx.db.insert("feedbackRecords", {
      ...args,
      createdAt: Date.now(),
    });
  },
});

// Get feedback by match ID
export const getByMatch = query({
  args: { matchId: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("feedbackRecords")
      .withIndex("by_match", (q) => q.eq("matchId", args.matchId))
      .collect();
  },
});

// Get single feedback by match ID
export const getByMatchId = query({
  args: { matchId: v.string() },
  handler: async (ctx, args) => {
    return await ctx.db
      .query("feedbackRecords")
      .withIndex("by_match", (q) => q.eq("matchId", args.matchId))
      .first();
  },
});

// Get feedback by date range
export const getByDateRange = query({
  args: {
    start: v.number(),
    end: v.number(),
  },
  handler: async (ctx, args) => {
    const all = await ctx.db.query("feedbackRecords").collect();
    return all.filter(
      (f) => f.createdAt >= args.start && f.createdAt <= args.end
    );
  },
});

// Get feedback stats for weight learning
export const getStats = query({
  args: {
    start: v.number(),
    end: v.number(),
  },
  handler: async (ctx, args) => {
    const all = await ctx.db.query("feedbackRecords").collect();
    const inRange = all.filter(
      (f) => f.createdAt >= args.start && f.createdAt <= args.end
    );

    const confirmed = inRange.filter((f) => f.confirmed);
    const rejected = inRange.filter((f) => !f.confirmed);

    const avg = (arr: number[]) =>
      arr.length ? arr.reduce((a, b) => a + b, 0) / arr.length : 0;

    return {
      totalFeedback: inRange.length,
      confirmedCount: confirmed.length,
      rejectedCount: rejected.length,
      avgConfirmedScore: avg(confirmed.map((f) => f.totalScore)),
      avgRejectedScore: avg(rejected.map((f) => f.totalScore)),
      confirmationRate:
        inRange.length > 0 ? confirmed.length / inRange.length : 0,
      confirmedAvgMedication: avg(confirmed.map((f) => f.medicationScore)),
      rejectedAvgMedication: avg(rejected.map((f) => f.medicationScore)),
      medicationDiff:
        avg(confirmed.map((f) => f.medicationScore)) -
        avg(rejected.map((f) => f.medicationScore)),
      confirmedAvgDosage: avg(confirmed.map((f) => f.dosageScore)),
      rejectedAvgDosage: avg(rejected.map((f) => f.dosageScore)),
      dosageDiff:
        avg(confirmed.map((f) => f.dosageScore)) -
        avg(rejected.map((f) => f.dosageScore)),
      confirmedAvgQuantity: avg(confirmed.map((f) => f.quantityScore)),
      rejectedAvgQuantity: avg(rejected.map((f) => f.quantityScore)),
      quantityDiff:
        avg(confirmed.map((f) => f.quantityScore)) -
        avg(rejected.map((f) => f.quantityScore)),
      confirmedAvgPrice: avg(confirmed.map((f) => f.priceScore)),
      rejectedAvgPrice: avg(rejected.map((f) => f.priceScore)),
      priceDiff:
        avg(confirmed.map((f) => f.priceScore)) -
        avg(rejected.map((f) => f.priceScore)),
      confirmedAvgRecency: avg(confirmed.map((f) => f.recencyScore)),
      rejectedAvgRecency: avg(rejected.map((f) => f.recencyScore)),
      recencyDiff:
        avg(confirmed.map((f) => f.recencyScore)) -
        avg(rejected.map((f) => f.recencyScore)),
      confirmedAvgTotal: avg(confirmed.map((f) => f.totalScore)),
      rejectedAvgTotal: avg(rejected.map((f) => f.totalScore)),
    };
  },
});

// Count all feedback
export const count = query({
  args: {},
  handler: async (ctx) => {
    const all = await ctx.db.query("feedbackRecords").collect();
    return all.length;
  },
});
