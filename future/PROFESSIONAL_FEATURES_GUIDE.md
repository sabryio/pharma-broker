# Professional Features Implementation Guide

> **Target Architecture**: Rust Core  
> **Last Updated**: December 22, 2025

Detailed implementation plans for key features to enhance PharmaBroker's functionality.

---

## Feature 1: Supplier Rating System ⭐

### Overview

Enable users to rate suppliers after confirmed matches, building trust and accountability.

### Data Model (Rust)

```rust
// core/src/domain/rating.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplierRating {
    pub id: String,
    pub supplier_jid: String,
    pub supplier_name: String,
    pub match_id: String,
    pub rater_jid: String,

    // Rating dimensions (1-5)
    pub quality_score: i32,
    pub delivery_score: i32,
    pub communication_score: i32,
    pub price_score: i32,
    pub overall_score: f64,

    pub comment: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplierProfile {
    pub jid: String,
    pub name: String,
    pub phone: String,
    pub avg_rating: f64,
    pub total_ratings: i32,
    pub confirmed_deals: i32,
    pub joined_at: DateTime<Utc>,
    pub verified: bool,
    pub verification_level: VerificationLevel, // Basic, Verified, Premium
}
```

### Implementation Tasks

- [ ] Create `core/crates/db/src/entity/rating.rs` - Rating entity
- [ ] Create `core/crates/db/src/repo/rating.rs` - Rating repository
- [ ] Create `core/crates/db/src/repo/supplier.rs` - Supplier profiles
- [ ] Add API handlers in `core/src/api/handlers.rs`:
  - [ ] `POST /api/ratings` - Submit rating
  - [ ] `GET /api/suppliers/{jid}/ratings` - Get ratings
  - [ ] `GET /api/suppliers/{jid}/profile` - Get profile
- [ ] Add rating prompt after match confirmation (24h delay)
- [ ] Dashboard: Supplier leaderboard component

### Rating Calculation

```rust
impl SupplierRating {
    pub fn calculate_overall(&self) -> f64 {
        const QUALITY_WEIGHT: f64 = 0.30;
        const DELIVERY_WEIGHT: f64 = 0.25;
        const COMMUNICATION_WEIGHT: f64 = 0.20;
        const PRICE_WEIGHT: f64 = 0.25;

        (self.quality_score as f64) * QUALITY_WEIGHT +
        (self.delivery_score as f64) * DELIVERY_WEIGHT +
        (self.communication_score as f64) * COMMUNICATION_WEIGHT +
        (self.price_score as f64) * PRICE_WEIGHT
    }
}
```

---

## Feature 2: Smart Push Notifications 🔔

### Overview

Real-time alerts via WhatsApp, Telegram, and browser push.

### Notification Types

| Type           | Trigger           | Priority | Channels     |
| -------------- | ----------------- | -------- | ------------ |
| New Match      | Score ≥ threshold | High     | WA, TG, Push |
| Urgent Request | Marked urgent     | Critical | WA, TG, Push |
| Price Drop     | Price below watch | Medium   | TG, Push     |
| Match Expiring | Pending > 24h     | Medium   | WA, TG       |
| Daily Digest   | 9:00 AM           | Low      | TG, Email    |

### Data Model (Rust)

```rust
// core/src/domain/notification.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreference {
    pub user_jid: String,
    pub whatsapp_enabled: bool,
    pub telegram_enabled: bool,
    pub telegram_chat_id: Option<String>,
    pub push_enabled: bool,
    pub email_enabled: bool,
    pub email: Option<String>,

    pub new_match_threshold: f64,
    pub urgent_only: bool,
    pub quiet_hours_start: Option<String>,
    pub quiet_hours_end: Option<String>,
    pub watched_medications: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub user_jid: String,
    pub notification_type: NotificationType,
    pub title: String,
    pub body: String,
    pub data: HashMap<String, String>,
    pub channels: Vec<Channel>,
    pub status: NotificationStatus,
    pub sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
```

### Implementation Tasks

- [ ] Create `core/src/notify/dispatcher.rs` - Multi-channel dispatcher
- [ ] Create `core/src/notify/templates.rs` - Message templates (AR/EN)
- [ ] Add preferences API endpoints
- [ ] Add quiet hours logic
- [ ] Integrate with WebSocket for real-time push

---

## Feature 3: Price Analytics 📊

### Overview

Track price history, identify trends, provide fair price recommendations.

### Data Model (Rust)

```rust
// core/src/domain/price.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub id: String,
    pub medication: String,
    pub price: f64,
    pub quantity: f64,
    pub unit_price: f64,
    pub source: PriceSource, // Offer or Request
    pub group_jid: String,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceAnalytics {
    pub medication: String,
    pub current_price: f64,
    pub avg_price_7d: f64,
    pub avg_price_30d: f64,
    pub min_price_30d: f64,
    pub max_price_30d: f64,
    pub price_change_7d: f64,
    pub price_change_30d: f64,
    pub trend_direction: TrendDirection, // Up, Down, Stable
    pub fair_price: f64,
    pub data_points: i32,
    pub last_updated: DateTime<Utc>,
}
```

### Fair Price Algorithm

```rust
pub fn calculate_fair_price(prices: &[f64]) -> f64 {
    if prices.len() < 3 {
        return average(prices);
    }

    let mut sorted = prices.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let q1 = percentile(&sorted, 25.0);
    let q3 = percentile(&sorted, 75.0);
    let iqr = q3 - q1;

    // Filter outliers
    let filtered: Vec<f64> = sorted
        .iter()
        .filter(|&&p| p >= q1 - 1.5 * iqr && p <= q3 + 1.5 * iqr)
        .copied()
        .collect();

    median(&filtered)
}
```

### Implementation Tasks

- [ ] Create `core/src/analytics/price.rs` - Price analysis
- [ ] Auto-record prices from offers/requests
- [ ] Calculate rolling averages and trends
- [ ] Add API endpoints:
  - [ ] `GET /api/analytics/price/{medication}`
  - [ ] `GET /api/analytics/trending`

---

## Feature 4: Batch & Expiry Tracking 📦

### Overview

Track batch numbers and expiry dates for compliance.

### Expiry Status Logic

```rust
pub fn get_expiry_status(expiry_date: DateTime<Utc>) -> ExpiryStatus {
    let now = Utc::now();
    let months = (expiry_date - now).num_days() / 30;

    match months {
        m if m <= 0 => ExpiryStatus::Expired,    // ⛔
        m if m <= 3 => ExpiryStatus::Critical,   // 🔴
        m if m <= 6 => ExpiryStatus::Warning,    // 🟡
        _ => ExpiryStatus::Ok,                    // 🟢
    }
}
```

### Implementation Tasks

- [ ] Extend offer entity with batch info
- [ ] Update AI prompt to extract batch/expiry
- [ ] Add expiry filter to offers API
- [ ] Create expiry warning notifications

---

## Feature 5: PWA / Mobile App 📱

### Overview

Progressive Web App for mobile-first experience.

### PWA Features

| Feature            | Priority |
| ------------------ | -------- |
| Installable        | High     |
| Offline mode       | High     |
| Push notifications | High     |
| Background sync    | Medium   |
| Camera (QR scan)   | Low      |

### Implementation Tasks

- [ ] Create `manifest.json`
- [ ] Create service worker
- [ ] Implement IndexedDB for offline
- [ ] Mobile-optimized UI components

---

## Implementation Priority

| Phase | Feature             | Weeks | Dependencies     |
| ----- | ------------------- | ----- | ---------------- |
| 1     | Supplier Ratings    | 2     | None             |
| 2     | Smart Notifications | 2     | notify module    |
| 3     | Price Analytics     | 2     | None             |
| 4     | Batch/Expiry        | 1     | AI prompt update |
| 5     | PWA                 | 2     | Frontend build   |

**Total: ~9 weeks**

---

## Success Metrics

| Feature          | KPI             | Target           |
| ---------------- | --------------- | ---------------- |
| Supplier Ratings | Submission rate | 30% of matches   |
| Notifications    | Open rate       | > 40%            |
| Price Analytics  | Usage           | 50+ queries/day  |
| Batch/Expiry     | Compliance      | 100% with expiry |
| PWA              | Installs        | 500+             |

---

_Last Updated: December 22, 2025_
