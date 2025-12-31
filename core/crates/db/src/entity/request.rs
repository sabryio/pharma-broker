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
    pub id: Uuid,
    pub raw_message_id: Uuid,
    pub participant_id: Uuid,
    pub group_id: Uuid,
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
    pub master_medication_id: Option<Uuid>,  // FK to medication_master for deterministic matching
    pub medication_curated: bool,            // Whether medication has been curated
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::raw_message::Entity",
        from = "Column::RawMessageId",
        to = "super::raw_message::Column::Id",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    RawMessage,
    #[sea_orm(
        belongs_to = "super::participant::Entity",
        from = "Column::ParticipantId",
        to = "super::participant::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Participant,
    #[sea_orm(
        belongs_to = "super::group::Entity",
        from = "Column::GroupId",
        to = "super::group::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Group,
    #[sea_orm(has_many = "super::match_::Entity")]
    Matches,
}

impl Related<super::raw_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RawMessage.def()
    }
}

impl Related<super::participant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Participant.def()
    }
}

impl Related<super::group::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Group.def()
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
            id: Uuid::new_v4(),
            raw_message_id: Uuid::new_v4(),
            participant_id: Uuid::nil(),
            group_id: Uuid::nil(),
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
            master_medication_id: None,
            medication_curated: false,
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
    raw_message_id: Uuid,
    medication: String,
    participant_id: Uuid,
    group_id: Uuid,
    // Optional fields
    medication_raw: Option<String>,
    quantity: Option<Decimal>,
    unit: Option<String>,
    max_price: Option<Decimal>,
    currency: Option<String>,
    urgency_level: UrgencyLevel,
    expiry_requirement: Option<String>,
    ai_confidence: f64,
    notes: Option<String>,
}

impl RequestBuilder {
    /// Create a new builder with required fields
    pub fn new(
        raw_message_id: Uuid,
        medication: impl Into<String>,
        participant_id: Uuid,
        group_id: Uuid,
    ) -> Self {
        let medication = medication.into();
        Self {
            raw_message_id,
            medication: medication.clone(),
            participant_id,
            group_id,
            medication_raw: Some(medication),
            quantity: None,
            unit: None,
            max_price: None,
            currency: Some("EGP".to_string()),
            urgency_level: UrgencyLevel::Normal,
            expiry_requirement: None,
            ai_confidence: 0.0,
            notes: None,
        }
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

    /// Set expiry requirement text
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

    /// Build the Request entity
    pub fn build(self) -> Model {
        use chrono::Utc;
        let now = Utc::now();

        Model {
            id: Uuid::new_v4(),
            raw_message_id: self.raw_message_id,
            participant_id: self.participant_id,
            group_id: self.group_id,
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
            content_embedding: None,
            master_medication_id: None,
            medication_curated: false,
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
        let msg_id = Uuid::new_v4();
        let part_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let request = RequestBuilder::new(msg_id, "Aspirin 100mg", part_id, group_id).build();

        assert_eq!(request.medication, "Aspirin 100mg");
        assert_eq!(request.participant_id, part_id);
        assert_eq!(request.group_id, group_id);
        assert_eq!(request.status, Status::Active);
    }

    #[test]
    fn test_request_builder_full() {
        let msg_id = Uuid::new_v4();
        let part_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let request = RequestBuilder::new(msg_id, "Ozempic 1mg", part_id, group_id)
            .quantity(2.0)
            .max_price(2000.0)
            .unit("boxes")
            .urgency_level(UrgencyLevel::Urgent)
            .ai_confidence(0.9)
            .notes("Urgent request")
            .build();

        assert_eq!(request.medication, "Ozempic 1mg");
        assert_eq!(request.participant_id, part_id);
        assert!(request.quantity.is_some());
        assert!(request.max_price.is_some());
        assert_eq!(request.urgency_level, UrgencyLevel::Urgent);
        assert!((request.ai_confidence - 0.9).abs() < 0.001);
    }
}
