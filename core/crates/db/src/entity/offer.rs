//! Offer entity - Medication supply offers
//!
//! Represents medication supply offers from WhatsApp messages.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export common types for backward compatibility
pub use super::common::{ItemStatus as Status, UrgencyLevel};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "offers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub raw_message_id: String,
    pub source_phone: String,
    pub source_name: Option<String>,
    pub source_group: String,
    pub group_name: String,
    pub medication: String,
    pub medication_raw: String,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub quantity: Option<Decimal>,
    pub unit: Option<String>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub price: Option<Decimal>,
    pub currency: Option<String>,
    pub expiry_date: Option<Date>,
    pub batch_number: Option<String>,
    pub notes: Option<String>,
    pub raw_message: Option<String>,
    pub status: Status,
    /// Deprecated: Use urgency_level instead
    pub urgent: bool,
    pub urgency_level: UrgencyLevel,
    pub expiry_info: Option<String>,
    pub ai_confidence: f64,
    pub content_embedding: Option<PgVector>, // Vector(384) for semantic search
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::raw_message::Entity",
        from = "Column::RawMessageId",
        to = "super::raw_message::Column::Id"
    )]
    RawMessage,
    #[sea_orm(has_many = "super::match_::Entity")]
    Matches,
}

impl Related<super::raw_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RawMessage.def()
    }
}

impl Related<super::match_::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Matches.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Default for Model {
    fn default() -> Self {
        use chrono::Utc;
        Self {
            id: String::new(),
            raw_message_id: String::new(),
            source_phone: String::new(),
            source_name: None,
            source_group: String::new(),
            group_name: String::new(),
            medication: String::new(),
            medication_raw: String::new(),
            quantity: None,
            unit: None,
            price: None,
            currency: None,
            expiry_date: None,
            batch_number: None,
            notes: None,
            raw_message: None,
            status: Status::Active,
            urgent: false,
            urgency_level: UrgencyLevel::Normal,
            expiry_info: None,
            ai_confidence: 0.0,
            content_embedding: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl Model {
    /// Get quantity as f64
    pub fn quantity_f64(&self) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        self.quantity
            .as_ref()
            .and_then(|d| d.to_f64())
            .unwrap_or(0.0)
    }

    /// Get price as f64
    pub fn price_f64(&self) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        self.price.as_ref().and_then(|d| d.to_f64()).unwrap_or(0.0)
    }

    /// Get source_name or empty string
    pub fn source_name_str(&self) -> &str {
        self.source_name.as_deref().unwrap_or("")
    }

    /// Get embedding as Vec<f32> if present
    pub fn get_embedding(&self) -> Option<Vec<f32>> {
        self.content_embedding.as_ref().map(|v| v.to_vec())
    }

    /// Check if this offer is urgent (any urgency level above Normal)
    pub fn is_urgent(&self) -> bool {
        self.urgency_level.is_urgent()
    }
}

// =============================================================================
// Builder Pattern
// =============================================================================

/// Builder for creating Offer entities with a fluent API
///
/// # Example
/// ```ignore
/// let offer = OfferBuilder::new("msg-123", "Aspirin 100mg", "+201234567890", "group@g.us")
///     .quantity(10.0)
///     .price(50.0)
///     .urgency_level(UrgencyLevel::Urgent)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct OfferBuilder {
    raw_message_id: String,
    medication: String,
    source_phone: String,
    source_group: String,
    // Optional fields
    source_name: Option<String>,
    group_name: Option<String>,
    medication_raw: Option<String>,
    quantity: Option<Decimal>,
    unit: Option<String>,
    price: Option<Decimal>,
    currency: Option<String>,
    expiry_date: Option<Date>,
    batch_number: Option<String>,
    notes: Option<String>,
    raw_message: Option<String>,
    urgency_level: UrgencyLevel,
    expiry_info: Option<String>,
    ai_confidence: f64,
    content_embedding: Option<PgVector>,
}

impl OfferBuilder {
    /// Create a new builder with required fields
    pub fn new(
        raw_message_id: impl Into<String>,
        medication: impl Into<String>,
        source_phone: impl Into<String>,
        source_group: impl Into<String>,
    ) -> Self {
        let medication = medication.into();
        Self {
            raw_message_id: raw_message_id.into(),
            medication: medication.clone(),
            source_phone: source_phone.into(),
            source_group: source_group.into(),
            source_name: None,
            group_name: None,
            medication_raw: Some(medication),
            quantity: None,
            unit: None,
            price: None,
            currency: Some("EGP".to_string()),
            expiry_date: None,
            batch_number: None,
            notes: None,
            raw_message: None,
            urgency_level: UrgencyLevel::Normal,
            expiry_info: None,
            ai_confidence: 0.0,
            content_embedding: None,
        }
    }

    /// Set the source name
    pub fn source_name(mut self, name: impl Into<String>) -> Self {
        self.source_name = Some(name.into());
        self
    }

    /// Set the group name
    pub fn group_name(mut self, name: impl Into<String>) -> Self {
        self.group_name = Some(name.into());
        self
    }

    /// Set the original medication text
    pub fn medication_raw(mut self, raw: impl Into<String>) -> Self {
        self.medication_raw = Some(raw.into());
        self
    }

    /// Set the quantity
    pub fn quantity(mut self, qty: f64) -> Self {
        use rust_decimal::prelude::FromPrimitive;
        self.quantity = Decimal::from_f64(qty);
        self
    }

    /// Set the quantity with Decimal
    pub fn quantity_decimal(mut self, qty: Decimal) -> Self {
        self.quantity = Some(qty);
        self
    }

    /// Set the unit
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Set the price
    pub fn price(mut self, price: f64) -> Self {
        use rust_decimal::prelude::FromPrimitive;
        self.price = Decimal::from_f64(price);
        self
    }

    /// Set the price with Decimal
    pub fn price_decimal(mut self, price: Decimal) -> Self {
        self.price = Some(price);
        self
    }

    /// Set the currency
    pub fn currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = Some(currency.into());
        self
    }

    /// Set the expiry date
    pub fn expiry_date(mut self, date: Date) -> Self {
        self.expiry_date = Some(date);
        self
    }

    /// Set the batch number
    pub fn batch_number(mut self, batch: impl Into<String>) -> Self {
        self.batch_number = Some(batch.into());
        self
    }

    /// Set notes
    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    /// Set the raw message content
    pub fn raw_message(mut self, msg: impl Into<String>) -> Self {
        self.raw_message = Some(msg.into());
        self
    }

    /// Set the urgency level
    pub fn urgency_level(mut self, level: UrgencyLevel) -> Self {
        self.urgency_level = level;
        self
    }

    /// Set as urgent (shorthand for urgency_level(Urgent))
    pub fn urgent(mut self) -> Self {
        self.urgency_level = UrgencyLevel::Urgent;
        self
    }

    /// Set expiry info text
    pub fn expiry_info(mut self, info: impl Into<String>) -> Self {
        self.expiry_info = Some(info.into());
        self
    }

    /// Set AI confidence score
    pub fn ai_confidence(mut self, confidence: f64) -> Self {
        self.ai_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set content embedding
    pub fn embedding(mut self, embedding: Vec<f32>) -> Self {
        self.content_embedding = Some(PgVector::from(embedding));
        self
    }

    /// Build the Offer entity
    pub fn build(self) -> Model {
        use chrono::Utc;
        let now = Utc::now();

        Model {
            id: uuid::Uuid::new_v4().to_string(),
            raw_message_id: self.raw_message_id,
            source_phone: self.source_phone,
            source_name: self.source_name,
            source_group: self.source_group.clone(),
            group_name: self.group_name.unwrap_or(self.source_group),
            medication: self.medication.clone(),
            medication_raw: self.medication_raw.unwrap_or(self.medication),
            quantity: self.quantity,
            unit: self.unit,
            price: self.price,
            currency: self.currency,
            expiry_date: self.expiry_date,
            batch_number: self.batch_number,
            notes: self.notes,
            raw_message: self.raw_message,
            status: Status::Active,
            urgent: self.urgency_level.is_urgent(),
            urgency_level: self.urgency_level,
            expiry_info: self.expiry_info,
            ai_confidence: self.ai_confidence,
            content_embedding: self.content_embedding,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offer_builder_minimal() {
        let offer =
            OfferBuilder::new("msg-1", "Aspirin 100mg", "+201234567890", "group@g.us").build();

        assert_eq!(offer.medication, "Aspirin 100mg");
        assert_eq!(offer.source_phone, "+201234567890");
        assert_eq!(offer.status, Status::Active);
        assert!(!offer.urgent);
    }

    #[test]
    fn test_offer_builder_full() {
        let offer = OfferBuilder::new("msg-2", "Ozempic 1mg", "+201111111111", "pharma@g.us")
            .source_name("Dr. Ahmed")
            .group_name("Pharma Exchange")
            .quantity(5.0)
            .price(1500.0)
            .unit("boxes")
            .urgent()
            .ai_confidence(0.95)
            .notes("Original packaging")
            .build();

        assert_eq!(offer.medication, "Ozempic 1mg");
        assert_eq!(offer.source_name, Some("Dr. Ahmed".to_string()));
        assert_eq!(offer.group_name, "Pharma Exchange");
        assert!(offer.quantity.is_some());
        assert!(offer.price.is_some());
        assert_eq!(offer.unit, Some("boxes".to_string()));
        assert!(offer.urgent);
        assert_eq!(offer.urgency_level, UrgencyLevel::Urgent);
        assert!((offer.ai_confidence - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_offer_is_urgent() {
        let normal = Model::default();
        assert!(!normal.is_urgent());

        let urgent = OfferBuilder::new("m", "med", "p", "g")
            .urgency_level(UrgencyLevel::Critical)
            .build();
        assert!(urgent.is_urgent());
    }
}
