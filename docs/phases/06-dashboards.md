# Phase 6: Grafana Dashboards

## Overview

Pre-built Grafana dashboards for monitoring PharmaBroker metrics.

## Dashboard Panels

```mermaid
graph TB
    subgraph "Main Dashboard"
        M1[Message Throughput]
        M2[AI Parse Success Rate]
        M3[Match Confirmation Rate]
        M4[Active Connections]
    end

    subgraph "Performance Dashboard"
        P1[Message Processing Time]
        P2[AI Parse Latency]
        P3[DB Query Duration]
        P4[Queue Depth]
    end

    subgraph "Alerts"
        A1[High Error Rate]
        A2[AI Timeout]
        A3[DB Connection Lost]
    end

    PROM[Prometheus] --> M1
    PROM --> M2
    PROM --> M3
    PROM --> M4
    PROM --> P1
    PROM --> P2
    PROM --> P3
    PROM --> P4
```

## Key Metrics

| Panel             | Metric                                       | Query                             |
| ----------------- | -------------------------------------------- | --------------------------------- |
| Message Rate      | `pharma_messages_received_total`             | `rate(...[5m])`                   |
| AI Success        | `pharma_ai_parse_total`                      | `rate(...{status="success"}[5m])` |
| Confirmation Rate | `pharma_match_confirmation_rate`             | Direct gauge                      |
| Processing P99    | `pharma_message_processing_duration_seconds` | `histogram_quantile(0.99, ...)`   |

## Sample Dashboard JSON

```json
{
  "title": "PharmaBroker Overview",
  "panels": [
    {
      "title": "Messages per Second",
      "type": "graph",
      "targets": [
        {
          "expr": "rate(pharma_messages_received_total[5m])"
        }
      ]
    }
  ]
}
```

## Verification

Manual verification required:

1. Import dashboard JSON to Grafana
2. Verify data source connection
3. Check panel queries return data
4. Validate alert thresholds
