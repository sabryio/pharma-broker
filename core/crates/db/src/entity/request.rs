//! Request entity - Medication demand requests
//!
//! Represents medication demand requests from WhatsApp messages.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export common types for backward compatibility
pub use super::common::{ItemStatus as Status, UrgencyLevel};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "requests")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub raw_message_id: String,
    pub source_phone: String,
    pub source_name: Option<String>,
    pub source_group: String,
    pub medication: String,
    pub medication_raw: String,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub quantity: Option<Decimal>,
    pub unit: Option<String>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub max_price: Option<Decimal>,
    pub currency: Option<String>,
    pub urgency_level: UrgencyLevel,
    pub expiry_requirement: Option<String>,
    pub ai_confidence: f64,
    pub notes: Option<String>,
    pub status: Status,
    pub content_embedding: Option<PgVector>, // Vector(768) for semantic search
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
            medication: String::new(),
            medication_raw: String::new(),
            quantity: None,
            unit: None,
            max_price: None,
            currency: None,
            urgency_level: UrgencyLevel::Normal,
            expiry_requirement: None,
            ai_confidence: 0.0,
            notes: None,
            status: Status::Active,
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

    /// Get max_price as f64
    pub fn max_price_f64(&self) -> f64 {
        use rust_decimal::prelude::ToPrimitive;
        self.max_price
            .as_ref()
            .and_then(|d| d.to_f64())
            .unwrap_or(0.0)
    }

    /// Get source_name or empty string
    pub fn source_name_str(&self) -> &str {
        self.source_name.as_deref().unwrap_or("")
    }

    /// Get embedding as Vec<f32> if present
    pub fn get_embedding(&self) -> Option<Vec<f32>> {
        self.content_embedding.as_ref().map(|v| v.to_vec())
    }

    /// Check if this request is urgent (any urgency level above Normal)
    pub fn is_urgent(&self) -> bool {
        self.urgency_level.is_urgent()
    }
}

// =============================================================================
// Builder Pattern
// =============================================================================

/// Builder for creating Request entities with a fluent API
///
/// # Example
/// ```ignore
/// let request = RequestBuilder::new("msg-123", "Ozempic 1mg", "+201234567890", "group@g.us")
///     .quantity(2.0)
///     .max_price(2000.0)
///     .urgency_level(UrgencyLevel::Critical)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct RequestBuilder {
    raw_message_id: String,
    medication: String,
    source_phone: String,
    source_group: String,
    // Optional fields
    source_name: Option<String>,
    medication_raw: Option<String>,
    quantity: Option<Decimal>,
    unit: Option<String>,
    max_price: Option<Decimal>,
    currency: Option<String>,
    urgency_level: UrgencyLevel,
    expiry_requirement: Option<String>,
    ai_confidence: f64,
    notes: Option<String>,
    content_embedding: Option<PgVector>,
}

impl RequestBuilder {
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
            medication_raw: Some(medication),
            quantity: None,
            unit: None,
            max_price: None,
            currency: Some("EGP".to_string()),
            urgency_level: UrgencyLevel::Normal,
            expiry_requirement: None,
            ai_confidence: 0.0,
            notes: None,
            content_embedding: None,
        }
    }

    /// Set the source name
    pub fn source_name(mut self, name: impl Into<String>) -> Self {
        self.source_name = Some(name.into());
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

    /// Set the maximum price
    pub fn max_price(mut self, price: f64) -> Self {
        use rust_decimal::prelude::FromPrimitive;
        self.max_price = Decimal::from_f64(price);
        self
    }

    /// Set the maximum price with Decimal
    pub fn max_price_decimal(mut self, price: Decimal) -> Self {
        self.max_price = Some(price);
        self
    }

    /// Set the currency
    pub fn currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = Some(currency.into());
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

    /// Set as critical (shorthand for urgency_level(Critical))
    pub fn critical(mut self) -> Self {
        self.urgency_level = UrgencyLevel::Critical;
        self
    }

    /// Set expiry requirement
    pub fn expiry_requirement(mut self, req: impl Into<String>) -> Self {
        self.expiry_requirement = Some(req.into());
        self
    }

    /// Set AI confidence score
    pub fn ai_confidence(mut self, confidence: f64) -> Self {
        self.ai_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set notes
    pub fn notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    /// Set content embedding
    pub fn embedding(mut self, embedding: Vec<f32>) -> Self {
        self.content_embedding = Some(PgVector::from(embedding));
        self
    }

    /// Build the Request entity
    pub fn build(self) -> Model {
        use chrono::Utc;
        let now = Utc::now();

        Model {
            id: uuid::Uuid::new_v4().to_string(),
            raw_message_id: self.raw_message_id,
            source_phone: self.source_phone,
            source_name: self.source_name,
            source_group: self.source_group,
            medication: self.medication.clone(),
            medication_raw: self.medication_raw.unwrap_or(self.medication),
            quantity: self.quantity,
            unit: self.unit,
            max_price: self.max_price,
            currency: self.currency,
            urgency_level: self.urgency_level,
            expiry_requirement: self.expiry_requirement,
            ai_confidence: self.ai_confidence,
            notes: self.notes,
            status: Status::Active,
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
    fn test_request_builder_minimal() {
        let request =
            RequestBuilder::new("msg-1", "Ozempic 1mg", "+201234567890", "group@g.us").build();

        assert_eq!(request.medication, "Ozempic 1mg");
        assert_eq!(request.source_phone, "+201234567890");
        assert_eq!(request.status, Status::Active);
        assert!(!request.is_urgent());
    }

    #[test]
    fn test_request_builder_full() {
        let request =
            RequestBuilder::new("msg-2", "Insulin Lantus", "+201111111111", "pharma@g.us")
                .source_name("Pharmacy ABC")
                .quantity(3.0)
                .max_price(500.0)
                .unit("pens")
                .critical()
                .expiry_requirement("At least 6 months")
                .ai_confidence(0.88)
                .notes("Urgent patient need")
                .build();

        assert_eq!(request.medication, "Insulin Lantus");
        assert_eq!(request.source_name, Some("Pharmacy ABC".to_string()));
        assert!(request.quantity.is_some());
        assert!(request.max_price.is_some());
        assert!(request.is_urgent());
        assert_eq!(request.urgency_level, UrgencyLevel::Critical);
        assert_eq!(
            request.expiry_requirement,
            Some("At least 6 months".to_string())
        );
    }

    #[test]
    fn test_request_is_urgent() {
        let normal = Model::default();
        assert!(!normal.is_urgent());

        let urgent = RequestBuilder::new("m", "med", "p", "g").urgent().build();
        assert!(urgent.is_urgent());
    }
}
