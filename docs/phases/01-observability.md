# Phase 1: Observability

## Overview

Metrics collection and health monitoring for production observability.

## Architecture

```mermaid
graph LR
    subgraph "Rust Core"
        M[metrics/mod.rs]
        H[api/handlers.rs]
    end

    subgraph "Endpoints"
        HE[/health]
        HR[/health/ready]
        HL[/health/live]
        ME[/metrics]
    end

    H --> HE
    H --> HR
    H --> HL
    M --> ME

    ME --> PROM[Prometheus]
    PROM --> GRAF[Grafana]
```

## Key Components

| File              | Function         | Description                    |
| ----------------- | ---------------- | ------------------------------ |
| `metrics/mod.rs`  | `init_metrics()` | Initialize Prometheus exporter |
| `metrics/mod.rs`  | `record_*()`     | Counter/histogram functions    |
| `api/handlers.rs` | `health_check()` | Basic liveness                 |
| `api/handlers.rs` | `health_ready()` | Readiness with DB check        |
| `api/handlers.rs` | `health_live()`  | Simple liveness probe          |

## Metrics Exposed

| Metric                                       | Type      | Labels        |
| -------------------------------------------- | --------- | ------------- |
| `pharma_messages_received_total`             | Counter   | group, status |
| `pharma_messages_processed_total`            | Counter   | type, status  |
| `pharma_offers_created_total`                | Counter   | -             |
| `pharma_requests_created_total`              | Counter   | -             |
| `pharma_ai_parse_total`                      | Counter   | status        |
| `pharma_message_processing_duration_seconds` | Histogram | -             |
| `pharma_ai_parse_duration_seconds`           | Histogram | -             |
| `pharma_active_grpc_connections`             | Gauge     | -             |
| `pharma_queue_size`                          | Gauge     | -             |

## Integration Test

```rust
#[tokio::test]
async fn test_phase1_observability() {
    // 1. Initialize metrics
    let handle = pharma_core::metrics::init_metrics();

    // 2. Record some metrics
    pharma_core::metrics::record_message_received("test-group", "success");
    pharma_core::metrics::record_offer_created();

    // 3. Verify metrics are exported
    let output = handle.render();
    assert!(output.contains("pharma_messages_received_total"));
    assert!(output.contains("pharma_offers_created_total"));
}

#[tokio::test]
async fn test_health_endpoints() {
    // Start server and check endpoints
    // GET /health → 200 {"status": "ok"}
    // GET /health/live → 200
    // GET /health/ready → 200 (if DB connected)
}
```
