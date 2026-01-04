/**
 * Domain error types using Effect's Data.TaggedError
 * Provides type-safe error handling throughout the application
 */
import { Data } from "effect";

// ============================================================================
// Not Found Errors
// ============================================================================

export class NotFoundError extends Data.TaggedError("NotFoundError")<{
  readonly entity: string;
  readonly id: string;
}> {
  get message() {
    return `${this.entity} with id ${this.id} not found`;
  }
}

export class OfferNotFoundError extends Data.TaggedError("OfferNotFoundError")<{
  readonly id: string;
}> {
  get message() {
    return `Offer with id ${this.id} not found`;
  }
}

export class RequestNotFoundError extends Data.TaggedError(
  "RequestNotFoundError"
)<{
  readonly id: string;
}> {
  get message() {
    return `Request with id ${this.id} not found`;
  }
}

export class MatchNotFoundError extends Data.TaggedError("MatchNotFoundError")<{
  readonly id: string;
}> {
  get message() {
    return `Match with id ${this.id} not found`;
  }
}

// ============================================================================
// Validation Errors
// ============================================================================

export class ValidationError extends Data.TaggedError("ValidationError")<{
  readonly field: string;
  readonly message: string;
  readonly value?: unknown;
}> {}

export class InvalidStatusTransitionError extends Data.TaggedError(
  "InvalidStatusTransitionError"
)<{
  readonly entity: string;
  readonly from: string;
  readonly to: string;
}> {
  get message() {
    return `Invalid ${this.entity} status transition from ${this.from} to ${this.to}`;
  }
}

// ============================================================================
// Database Errors
// ============================================================================

export class DatabaseError extends Data.TaggedError("DatabaseError")<{
  readonly operation: string;
  readonly cause: unknown;
}> {
  get message() {
    return `Database error during ${this.operation}: ${String(this.cause)}`;
  }
}

export class DuplicateEntityError extends Data.TaggedError(
  "DuplicateEntityError"
)<{
  readonly entity: string;
  readonly field: string;
  readonly value: string;
}> {
  get message() {
    return `${this.entity} with ${this.field} = ${this.value} already exists`;
  }
}

// ============================================================================
// AI/Parsing Errors
// ============================================================================

export class AiParseError extends Data.TaggedError("AiParseError")<{
  readonly text: string;
  readonly cause: unknown;
}> {
  get message() {
    return `Failed to parse message: ${String(this.cause)}`;
  }
}

export class AiTimeoutError extends Data.TaggedError("AiTimeoutError")<{
  readonly operation: string;
  readonly timeoutMs: number;
}> {
  get message() {
    return `AI ${this.operation} timed out after ${this.timeoutMs}ms`;
  }
}

export class EmbeddingError extends Data.TaggedError("EmbeddingError")<{
  readonly text: string;
  readonly cause: unknown;
}> {
  get message() {
    return `Failed to generate embedding: ${String(this.cause)}`;
  }
}
