# Professional Features Implementation Guide

Detailed implementation plans for key features to enhance PharmaBroker's functionality, user engagement, and competitiveness.

---

## Feature 1: Supplier Rating System ⭐

### Overview

Enable users to rate suppliers after confirmed matches, building trust and accountability.

### Data Model

```go
// internal/domain/rating.go
type SupplierRating struct {
    ID           string    `json:"id"`
    SupplierJID  string    `json:"supplier_jid"`
    SupplierName string    `json:"supplier_name"`
    MatchID      string    `json:"match_id"`
    RaterJID     string    `json:"rater_jid"`

    // Rating dimensions (1-5)
    QualityScore    int     `json:"quality_score"`    // Product quality
    DeliveryScore   int     `json:"delivery_score"`   // On-time delivery
    CommunicationScore int  `json:"communication_score"` // Responsiveness
    PriceScore      int     `json:"price_score"`      // Fair pricing
    OverallScore    float64 `json:"overall_score"`    // Calculated average

    Comment      string    `json:"comment,omitempty"`
    CreatedAt    time.Time `json:"created_at"`
}

type SupplierProfile struct {
    JID             string  `json:"jid"`
    Name            string  `json:"name"`
    Phone           string  `json:"phone"`
    AvgRating       float64 `json:"avg_rating"`
    TotalRatings    int     `json:"total_ratings"`
    ConfirmedDeals  int     `json:"confirmed_deals"`
    JoinedAt        time.Time `json:"joined_at"`
    Verified        bool    `json:"verified"`
    VerificationLevel string `json:"verification_level"` // basic, verified, premium
}
```

### Implementation Tasks

- [ ] Create `internal/domain/rating.go` - Rating models
- [ ] Create `internal/storage/rating_repo.go` - Rating repository
- [ ] Create `internal/storage/supplier_repo.go` - Supplier profiles
- [ ] Add API endpoints:
  - [ ] `POST /api/ratings` - Submit rating
  - [ ] `GET /api/suppliers/{jid}/ratings` - Get supplier ratings
  - [ ] `GET /api/suppliers/{jid}/profile` - Get supplier profile
- [ ] Add rating prompt after match confirmation (24h delay)
- [ ] Dashboard: Supplier leaderboard component
- [ ] Bot command: `/rate <match_id> <1-5>` or `/supplier <name>`

### Rating Calculation

```go
func CalculateOverallRating(r *SupplierRating) float64 {
    weights := map[string]float64{
        "quality":       0.30,
        "delivery":      0.25,
        "communication": 0.20,
        "price":         0.25,
    }
    return float64(r.QualityScore)*weights["quality"] +
           float64(r.DeliveryScore)*weights["delivery"] +
           float64(r.CommunicationScore)*weights["communication"] +
           float64(r.PriceScore)*weights["price"]
}
```

---

## Feature 2: Smart Push Notifications 🔔

### Overview

Real-time alerts via WhatsApp, Telegram, and browser push for critical events.

### Notification Types

| Type           | Trigger                              | Priority | Channels     |
| -------------- | ------------------------------------ | -------- | ------------ |
| New Match      | Match created with score ≥ threshold | High     | WA, TG, Push |
| Urgent Request | Request marked urgent                | Critical | WA, TG, Push |
| Price Drop     | Offer price drops for watched med    | Medium   | TG, Push     |
| Match Expiring | Pending match > 24h                  | Medium   | WA, TG       |
| Daily Digest   | 9:00 AM daily                        | Low      | TG, Email    |

### Data Model

```go
type NotificationPreference struct {
    UserJID           string   `json:"user_jid"`
    WhatsAppEnabled   bool     `json:"whatsapp_enabled"`
    TelegramEnabled   bool     `json:"telegram_enabled"`
    TelegramChatID    string   `json:"telegram_chat_id"`
    PushEnabled       bool     `json:"push_enabled"`
    EmailEnabled      bool     `json:"email_enabled"`
    Email             string   `json:"email"`

    // Granular controls
    NewMatchThreshold float64  `json:"new_match_threshold"` // Min score to notify
    UrgentOnly        bool     `json:"urgent_only"`
    QuietHoursStart   string   `json:"quiet_hours_start"` // "22:00"
    QuietHoursEnd     string   `json:"quiet_hours_end"`   // "08:00"
    WatchedMedications []string `json:"watched_medications"`
}

type Notification struct {
    ID         string    `json:"id"`
    UserJID    string    `json:"user_jid"`
    Type       string    `json:"type"`
    Title      string    `json:"title"`
    Body       string    `json:"body"`
    Data       map[string]string `json:"data"`
    Channels   []string  `json:"channels"` // ["whatsapp", "telegram", "push"]
    Status     string    `json:"status"`   // pending, sent, failed
    SentAt     *time.Time `json:"sent_at"`
    CreatedAt  time.Time `json:"created_at"`
}
```

### Implementation Tasks

- [ ] Create `internal/domain/notification.go` - Notification models
- [ ] Create `internal/notify/dispatcher.go` - Multi-channel dispatcher
- [ ] Create `internal/notify/templates.go` - Message templates (AR/EN)
- [ ] Create `internal/storage/notification_repo.go` - Notification storage
- [ ] Add Web Push support (VAPID keys)
- [ ] Add quiet hours logic
- [ ] Add preferences API:
  - [ ] `GET /api/notifications/preferences`
  - [ ] `PUT /api/notifications/preferences`
- [ ] Bot commands: `/notify on|off`, `/watch <medication>`
- [ ] Dashboard: Notification center component

### Message Templates

```go
var NotificationTemplates = map[string]map[string]string{
    "new_match": {
        "en": "🔔 New Match: {{.Medication}} ({{.Score}}%)\n💊 {{.Quantity}} units @ {{.Price}} EGP\nID: {{.ID}}",
        "ar": "🔔 مطابقة جديدة: {{.Medication}} ({{.Score}}%)\n💊 {{.Quantity}} وحدة @ {{.Price}} ج.م\nID: {{.ID}}",
    },
    "urgent_request": {
        "en": "🔥 URGENT: {{.Medication}} needed\n📋 {{.Quantity}} units requested\nGroup: {{.Group}}",
        "ar": "🔥 عاجل: مطلوب {{.Medication}}\n📋 {{.Quantity}} وحدة\nالمجموعة: {{.Group}}",
    },
}
```

---

## Feature 3: Price Analytics & Intelligence 📊

### Overview

Track price history, identify trends, and provide fair price recommendations.

### Data Model

```go
type PricePoint struct {
    ID          string    `json:"id"`
    Medication  string    `json:"medication"`
    Price       float64   `json:"price"`
    Quantity    float64   `json:"quantity"`
    UnitPrice   float64   `json:"unit_price"` // price / quantity
    Source      string    `json:"source"`     // "offer" or "request"
    GroupJID    string    `json:"group_jid"`
    RecordedAt  time.Time `json:"recorded_at"`
}

type PriceAnalytics struct {
    Medication     string  `json:"medication"`
    CurrentPrice   float64 `json:"current_price"`   // Latest avg
    AvgPrice7d     float64 `json:"avg_price_7d"`
    AvgPrice30d    float64 `json:"avg_price_30d"`
    MinPrice30d    float64 `json:"min_price_30d"`
    MaxPrice30d    float64 `json:"max_price_30d"`
    PriceChange7d  float64 `json:"price_change_7d"`  // % change
    PriceChange30d float64 `json:"price_change_30d"`
    TrendDirection string  `json:"trend_direction"` // up, down, stable
    FairPrice      float64 `json:"fair_price"`      // Recommended
    DataPoints     int     `json:"data_points"`
    LastUpdated    time.Time `json:"last_updated"`
}

type PriceAlert struct {
    ID          string  `json:"id"`
    UserJID     string  `json:"user_jid"`
    Medication  string  `json:"medication"`
    AlertType   string  `json:"alert_type"`   // below, above, change
    ThresholdPrice float64 `json:"threshold_price"`
    ThresholdPercent float64 `json:"threshold_percent"`
    Active      bool    `json:"active"`
    CreatedAt   time.Time `json:"created_at"`
}
```

### Implementation Tasks

- [ ] Create `internal/domain/price_analytics.go` - Price models
- [ ] Create `internal/storage/price_repo.go` - Price history storage
- [ ] Create `internal/analytics/price_analyzer.go` - Analysis logic
- [ ] Auto-record prices from offers/requests
- [ ] Calculate rolling averages and trends
- [ ] Fair price algorithm (median + IQR)
- [ ] Add API endpoints:
  - [ ] `GET /api/analytics/price/{medication}` - Price analytics
  - [ ] `GET /api/analytics/trending` - Top movers
  - [ ] `POST /api/alerts/price` - Create price alert
- [ ] Dashboard: Price chart component (Chart.js)
- [ ] Bot commands: `/price <medication>`, `/alert add <med> below <price>`

### Fair Price Algorithm

```go
func CalculateFairPrice(prices []float64) float64 {
    if len(prices) < 3 {
        return average(prices)
    }

    sort.Float64s(prices)
    q1 := percentile(prices, 25)
    q3 := percentile(prices, 75)
    iqr := q3 - q1

    // Filter outliers
    var filtered []float64
    for _, p := range prices {
        if p >= q1-1.5*iqr && p <= q3+1.5*iqr {
            filtered = append(filtered, p)
        }
    }

    return median(filtered)
}
```

---

## Feature 4: Batch & Expiry Tracking 📦

### Overview

Track medication batch numbers and expiry dates for regulatory compliance and quality assurance.

### Data Model

```go
type BatchInfo struct {
    BatchNumber   string    `json:"batch_number"`
    ExpiryDate    time.Time `json:"expiry_date"`
    ManufactureDate *time.Time `json:"manufacture_date,omitempty"`
    Manufacturer  string    `json:"manufacturer,omitempty"`
}

// Extend Offer model
type OfferWithBatch struct {
    domain.Offer
    BatchInfo     *BatchInfo `json:"batch_info,omitempty"`
    ExpiryMonths  int        `json:"expiry_months"` // Months until expiry
    ExpiryStatus  string     `json:"expiry_status"` // ok, warning, critical
}
```

### Expiry Status Logic

```go
func GetExpiryStatus(expiryDate time.Time) string {
    months := int(time.Until(expiryDate).Hours() / 24 / 30)
    switch {
    case months <= 0:
        return "expired"
    case months <= 3:
        return "critical" // 🔴
    case months <= 6:
        return "warning"  // 🟡
    default:
        return "ok"       // 🟢
    }
}
```

### Implementation Tasks

- [ ] Extend `domain.Offer` with BatchInfo
- [ ] Update AI prompt to extract batch/expiry from messages
- [ ] Add expiry parsing for formats: "exp 03/25", "انتهاء 2025"
- [ ] Create expiry warning notifications
- [ ] Add expiry filter to API:
  - [ ] `GET /api/offers?min_expiry=6` (minimum 6 months)
- [ ] Dashboard: Expiry indicator on offer cards
- [ ] Bot: Include expiry in search results

### AI Prompt Addition

```
## Batch & Expiry Extraction
Extract batch numbers and expiry dates if mentioned:
- "batch: ABC123, exp 03/2025" → batch_number: "ABC123", expiry: "2025-03-01"
- "انتهاء 6/2025" → expiry: "2025-06-01"
- "صلاحية سنة" → expiry: (current date + 1 year)
```

---

## Feature 5: PWA / Mobile App 📱

### Overview

Progressive Web App for mobile-first experience with offline capabilities.

### PWA Features

| Feature            | Status | Priority |
| ------------------ | ------ | -------- |
| Installable        | Todo   | High     |
| Offline mode       | Todo   | High     |
| Push notifications | Todo   | High     |
| Background sync    | Todo   | Medium   |
| Camera (QR scan)   | Todo   | Low      |

### Implementation Tasks

- [ ] Create `manifest.json` for PWA
- [ ] Create service worker for offline caching
- [ ] Implement IndexedDB for offline data
- [ ] Add "Add to Home Screen" prompt
- [ ] Implement background sync for offline actions
- [ ] Add push notification subscription
- [ ] Mobile-optimized UI components:
  - [ ] Bottom navigation bar
  - [ ] Swipe actions on cards
  - [ ] Pull-to-refresh
- [ ] QR scanner for medication lookup (optional)

### Service Worker Strategy

```javascript
// Cache strategies
const CACHE_NAME = "pharmabroker-v1";
const STATIC_ASSETS = ["/index.html", "/app.js", "/styles.css"];
const API_CACHE = "api-cache-v1";

// Cache-first for static assets
// Network-first for API calls
// Stale-while-revalidate for images
```

### manifest.json

```json
{
  "name": "PharmaBroker",
  "short_name": "PB",
  "description": "Pharmaceutical Trading Platform",
  "start_url": "/",
  "display": "standalone",
  "background_color": "#1a1a2e",
  "theme_color": "#4a90d9",
  "icons": [
    { "src": "/icon-192.png", "sizes": "192x192", "type": "image/png" },
    { "src": "/icon-512.png", "sizes": "512x512", "type": "image/png" }
  ]
}
```

---

## Implementation Priority & Timeline

| Phase | Feature             | Weeks | Dependencies        |
| ----- | ------------------- | ----- | ------------------- |
| 1     | Supplier Ratings    | 2     | None                |
| 2     | Smart Notifications | 2     | Existing notify pkg |
| 3     | Price Analytics     | 2     | None                |
| 4     | Batch/Expiry        | 1     | AI prompt update    |
| 5     | PWA                 | 2     | Frontend build      |

**Total estimated time: 9 weeks**

---

## Success Metrics

| Feature          | KPI                    | Target                   |
| ---------------- | ---------------------- | ------------------------ |
| Supplier Ratings | Rating submission rate | 30% of confirmed matches |
| Notifications    | Open rate              | > 40%                    |
| Price Analytics  | Usage                  | 50+ queries/day          |
| Batch/Expiry     | Compliance             | 100% offers with expiry  |
| PWA              | Installation           | 500+ installs            |

---

_Document Version: 1.0_  
_Last Updated: December 2024_
