# Phase 2: Reliability

## Overview

Queue-based processing, retry logic, and AI error handling for system resilience.

## Architecture

```mermaid
graph TB
    subgraph "Message Flow"
        MSG[Incoming Message]
        Q[In-Memory Queue]
        PROC[Processor]
    end

    subgraph "AI Integration"
        AI[AI Client]
        RETRY[Retry Executor]
        CB[Circuit Breaker]
    end

    MSG --> Q
    Q --> PROC
    PROC --> AI
    AI --> RETRY
    RETRY --> CB
    CB -->|success| DB[(PostgreSQL)]
    CB -->|failure| DLQ[Dead Letter]
```

## Key Components

| File           | Component         | Description                     |
| -------------- | ----------------- | ------------------------------- |
| `queue/mod.rs` | `BoundedQueue<T>` | Thread-safe bounded queue       |
| `retry/mod.rs` | `RetryExecutor`   | Exponential backoff with jitter |
| `ai/client.rs` | `AiClient`        | AI gateway HTTP client          |
| `ai/retry.rs`  | Circuit breaker   | Prevent cascading failures      |

## Queue Configuration

```rust
pub struct QueueConfig {
    pub capacity: usize,        // Default: 1000
    pub batch_size: usize,      // Default: 10
    pub drain_timeout: Duration, // Default: 30s
}
```

## Retry Behavior

```mermaid
sequenceDiagram
    participant C as Client
    participant R as RetryExecutor
    participant AI as AI Gateway

    C->>R: Execute operation
    R->>AI: Attempt 1
    AI-->>R: 500 Error
    Note over R: Wait 1s + jitter
    R->>AI: Attempt 2
    AI-->>R: 500 Error
    Note over R: Wait 2s + jitter
    R->>AI: Attempt 3
    AI-->>R: 200 OK
    R-->>C: Success
```

## Integration Test

```rust
#[tokio::test]
async fn test_phase2_queue_reliability() {
    let queue = BoundedQueue::new(100);

    // Push items
    for i in 0..50 {
        queue.push(format!("msg-{}", i)).await;
    }

    // Drain and verify
    let batch = queue.drain_batch(10, Duration::from_secs(1)).await;
    assert_eq!(batch.len(), 10);
}

#[tokio::test]
async fn test_retry_with_backoff() {
    let executor = RetryExecutor::new(RetryConfig::default());
    let attempt_count = Arc::new(AtomicU32::new(0));

    let result = executor.execute(|| async {
        attempt_count.fetch_add(1, Ordering::SeqCst);
        if attempt_count.load(Ordering::SeqCst) < 3 {
            Err(Error::transient("retry me"))
        } else {
            Ok("success")
        }
    }).await;

    assert!(result.is_ok());
    assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
}
```
