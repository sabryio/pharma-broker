# AI-Powered Conversational Bot System

> **Target Architecture**: Rust Core with MCP Integration  
> **Last Updated**: December 22, 2025

## Overview

A comprehensive system enabling natural language communication with PharmaBroker, powered by AI for intelligent task execution via MCP (Model Context Protocol).

```mermaid
flowchart TB
    subgraph Input["User Input Channels"]
        WA[WhatsApp] --> Gateway
        TG[Telegram] --> Gateway
        API[REST API] --> Gateway
    end

    subgraph Core["Rust AI Core Engine"]
        Gateway[Message Gateway] --> Auth[Authentication]
        Auth --> NLU[Natural Language Understanding]
        NLU --> Intent[Intent Classifier]
        Intent --> Context[Context Manager]
        Context --> Executor[Tool Executor]
    end

    subgraph MCP["MCP Tool Layer"]
        Executor --> Tools{MCP Tools}
        Tools --> DB[Database Tools]
        Tools --> Match[Matching Tools]
        Tools --> Report[Report Tools]
        Tools --> Config[Config Tools]
        Tools --> Notify[Notification Tools]
    end

    subgraph Response["Response Generation"]
        Executor --> ResponseGen[AI Response Generator]
        ResponseGen --> Format[Message Formatter]
        Format --> WA
        Format --> TG
    end
```

---

## Architecture Components

### 1. Message Gateway (Rust)

Unified entry point for all messaging platforms.

```rust
// core/src/bot/gateway.rs

pub struct MessageGateway {
    whatsapp_rx: mpsc::Receiver<UnifiedMessage>,
    telegram_rx: mpsc::Receiver<UnifiedMessage>,
    ai_engine: Arc<AIEngine>,
    sessions: Arc<RwLock<SessionManager>>,
}

#[derive(Debug, Clone)]
pub struct UnifiedMessage {
    pub platform: Platform,       // WhatsApp | Telegram | API
    pub user_id: String,          // Normalized user identifier
    pub chat_id: String,          // Chat/Group ID
    pub content: String,          // Raw message content
    pub reply_to: Option<String>, // Reply context
    pub timestamp: DateTime<Utc>,
    pub attachments: Vec<Attachment>,
}
```

### 2. Authentication Layer (Rust)

Secure multi-tier access control.

```rust
// core/src/bot/auth.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthLevel {
    Public = 0,    // Read-only stats
    Operator = 1,  // Confirm/reject matches
    Admin = 2,     // Config, user management
    Owner = 3,     // Full system control
}

pub struct UserSession {
    pub user_id: String,
    pub platform: Platform,
    pub auth_level: AuthLevel,
    pub preferences: UserPrefs,
    pub context: ConversationContext,
    pub last_active: DateTime<Utc>,
    pub rate_limiter: RateLimiter,
}
```

### 3. Natural Language Understanding (Rust + LLM)

AI-powered intent recognition and entity extraction.

```rust
// core/src/bot/nlu.rs

pub struct NLUEngine {
    ai_client: Arc<AiClient>,
    context_window: VecDeque<Message>,  // Last N messages
}

#[derive(Debug, Clone)]
pub struct ParsedIntent {
    pub intent: IntentType,
    pub confidence: f64,
    pub entities: HashMap<String, String>,
    pub raw_query: String,
}
```

**Supported Intents:**

| Intent              | Examples                             | Entities         |
| ------------------- | ------------------------------------ | ---------------- |
| `greet`             | "hello", "hi", "مرحبا"               | -                |
| `status`            | "how's the system?", "ازاي النظام؟"  | -                |
| `confirm_match`     | "confirm abc123", "أكد المطابقة"     | match_id         |
| `reject_match`      | "reject abc123", "ارفض"              | match_id, reason |
| `search_medication` | "find Augmentin", "ابحث عن اوجمنتين" | medication       |
| `list_pending`      | "show pending", "اعرض المعلق"        | filter, limit    |
| `generate_report`   | "daily report", "تقرير اليوم"        | period           |
| `configure`         | "set threshold to 0.8"               | key, value       |
| `help`              | "what can you do?", "ممكن تعمل ايه؟" | topic            |

---

## MCP Tool Definitions

Model Context Protocol tools for AI task execution.

```json
{
  "tools": [
    {
      "name": "get_system_status",
      "description": "Get current system status including offers, requests, matches count",
      "parameters": {}
    },
    {
      "name": "search_medications",
      "description": "Search for offers or requests by medication name",
      "parameters": {
        "medication": { "type": "string" },
        "type": { "type": "string", "enum": ["offer", "request", "both"] },
        "limit": { "type": "integer", "default": 10 }
      }
    },
    {
      "name": "get_pending_matches",
      "description": "List pending matches optionally filtered",
      "parameters": {
        "urgent_only": { "type": "boolean", "default": false },
        "min_score": { "type": "number", "default": 0.5 },
        "limit": { "type": "integer", "default": 5 }
      }
    },
    {
      "name": "confirm_match",
      "description": "Confirm a pending match by ID",
      "parameters": {
        "match_id": { "type": "string" },
        "note": { "type": "string" }
      }
    },
    {
      "name": "reject_match",
      "description": "Reject a pending match by ID",
      "parameters": {
        "match_id": { "type": "string" },
        "reason": { "type": "string" }
      }
    },
    {
      "name": "generate_report",
      "description": "Generate summary report for a time period",
      "parameters": {
        "period": {
          "type": "string",
          "enum": ["today", "yesterday", "week", "month"]
        },
        "format": { "type": "string", "enum": ["text", "csv"] }
      }
    }
  ]
}
```

---

## Conversation Example

```
User: مرحبا
Bot: أهلاً! 👋 أنا بوت فارما بروكر. كيف أقدر أساعدك؟

User: عندي كام matching معلق؟
Bot: 📋 عندك 5 مطابقات معلقة:

1. *Augmentin 1g* (92%) 🔥
   ID: abc123
2. *Concor 5mg* (87%)
   ID: def456
...

هل تحب تأكد أي واحدة؟

User: اكد الأولى
Bot: ✅ تم تأكيد مطابقة Augmentin 1g (abc123)
   البائع: صيدلية النور
   المشتري: د. أحمد
```

---

## Security Model

### Permission Matrix

| Action           | Public | Operator | Admin | Owner |
| ---------------- | ------ | -------- | ----- | ----- |
| View stats       | ✅     | ✅       | ✅    | ✅    |
| Search           | ✅     | ✅       | ✅    | ✅    |
| List pending     | ❌     | ✅       | ✅    | ✅    |
| Confirm/Reject   | ❌     | ✅       | ✅    | ✅    |
| Generate reports | ❌     | ✅       | ✅    | ✅    |
| Manage groups    | ❌     | ❌       | ✅    | ✅    |
| Config changes   | ❌     | ❌       | ✅    | ✅    |
| User management  | ❌     | ❌       | ❌    | ✅    |

### Rate Limiting

```rust
pub struct RateLimits {
    pub messages_per_minute: u32,   // Default: 20
    pub tool_calls_per_minute: u32, // Default: 10
    pub reports_per_hour: u32,      // Default: 5
}
```

---

## Implementation Tasks

### Phase 1: Core Infrastructure

- [ ] Create `core/src/bot/gateway.rs` - Unified message gateway
- [ ] Create `core/src/bot/session.rs` - Session management
- [ ] Create `core/src/bot/auth.rs` - Authentication layer
- [ ] Create `core/src/bot/nlu.rs` - NLU engine wrapper
- [ ] Define MCP tool schemas in `core/src/bot/tools/`

### Phase 2: Intent & Entity Extraction

- [ ] Create intent classification prompt
- [ ] Implement entity extraction for medications, IDs, dates
- [ ] Add Arabic/English normalization
- [ ] Create intent test cases (50+ examples)

### Phase 3: MCP Tool Implementation

- [ ] `tools/status.rs` - System status tool
- [ ] `tools/search.rs` - Medication search tool
- [ ] `tools/matches.rs` - Match management tools
- [ ] `tools/reports.rs` - Report generation tool
- [ ] `tools/config.rs` - Configuration tool

### Phase 4: Platform Integration

- [ ] Extend WhatsApp bridge with bot commands
- [ ] Implement Telegram bot (optional)
- [ ] Add webhook endpoint for API access

---

## Code Structure

```
core/src/
├── bot/
│   ├── mod.rs            # Module exports
│   ├── gateway.rs        # Message gateway
│   ├── session.rs        # Session management
│   ├── auth.rs           # Authentication
│   ├── nlu.rs            # NLU engine
│   ├── executor.rs       # Tool executor
│   ├── response.rs       # Response generator
│   └── tools/
│       ├── mod.rs        # Tool registry
│       ├── schema.rs     # MCP tool definitions
│       ├── status.rs     # Status tool
│       ├── search.rs     # Search tool
│       ├── matches.rs    # Match tools
│       ├── reports.rs    # Report tools
│       └── config.rs     # Config tools
```

---

## AI Prompt Template

```
You are PharmaBroker Bot, an intelligent assistant for pharmaceutical trading.

## Your Capabilities:
- Search medications (offers and requests)
- Manage match confirmations/rejections
- Generate reports
- Configure system settings (admin only)
- Monitor WhatsApp groups

## Available Tools:
{{tools}}

## User Info:
- Platform: {{platform}}
- Auth Level: {{auth_level}}
- Language: {{language}}

## Conversation History:
{{context}}

## Current Message:
{{message}}

Respond naturally in {{language}}. Use the appropriate tool if needed.
If unsure, ask for clarification. Be helpful and professional.
```

---

_Document Version: 2.0 (Rust)_  
_Last Updated: December 22, 2025_
