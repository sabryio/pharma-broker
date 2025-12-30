/* eslint-disable */
/**
 * Generated `api` utility.
 *
 * THIS CODE IS AUTOMATICALLY GENERATED.
 *
 * To regenerate, run `npx convex dev`.
 * @module
 */

import type * as auditLogs from "../auditLogs.js";
import type * as feedback from "../feedback.js";
import type * as groups from "../groups.js";
import type * as matchQueue from "../matchQueue.js";
import type * as matches from "../matches.js";
import type * as medicationMappings from "../medicationMappings.js";
import type * as offers from "../offers.js";
import type * as rawMessages from "../rawMessages.js";
import type * as requests from "../requests.js";
import type * as reviewQueue from "../reviewQueue.js";
import type * as weightHistory from "../weightHistory.js";

import type {
  ApiFromModules,
  FilterApi,
  FunctionReference,
} from "convex/server";

declare const fullApi: ApiFromModules<{
  auditLogs: typeof auditLogs;
  feedback: typeof feedback;
  groups: typeof groups;
  matchQueue: typeof matchQueue;
  matches: typeof matches;
  medicationMappings: typeof medicationMappings;
  offers: typeof offers;
  rawMessages: typeof rawMessages;
  requests: typeof requests;
  reviewQueue: typeof reviewQueue;
  weightHistory: typeof weightHistory;
}>;

/**
 * A utility for referencing Convex functions in your app's public API.
 *
 * Usage:
 * ```js
 * const myFunctionReference = api.myModule.myFunction;
 * ```
 */
export declare const api: FilterApi<
  typeof fullApi,
  FunctionReference<any, "public">
>;

/**
 * A utility for referencing Convex functions in your app's internal API.
 *
 * Usage:
 * ```js
 * const myFunctionReference = internal.myModule.myFunction;
 * ```
 */
export declare const internal: FilterApi<
  typeof fullApi,
  FunctionReference<any, "internal">
>;

export declare const components: {};
