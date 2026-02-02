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
    pub price: Option<Decimal>,
    pub currency: Option<String>,
    // expiry_date removed - redundant with expiry_info
    pub batch_number: Option<String>,
    pub notes: Option<String>,
    pub status: Status,
    pub urgency_level: UrgencyLevel,
    pub expiry_info: Option<String>,
    pub ai_confidence: f64,
    pub content_embedding: Option<PgVector>, // Vector(768) for semantic search
    pub master_medication_id: Option<Uuid>,  // FK to medication_master for deterministic matching
    pub medication_curated: bool,            // Whether medication has been curated
    pub confirmed_match_count: i32,          // Number of confirmed matches for this offer
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
            price: None,
            currency: None,
            // expiry_date removed
            batch_number: None,
            notes: None,
            status: Status::Active,
            urgency_level: UrgencyLevel::Normal,
            expiry_info: None,
            ai_confidence: 0.0,
            content_embedding: None,
            master_medication_id: None,
            medication_curated: false,
            confirmed_match_count: 0,
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
    raw_message_id: Uuid,
    medication: String,
    participant_id: Uuid,
    group_id: Uuid,
    // Optional fields
    medication_raw: Option<String>,
    quantity: Option<Decimal>,
    unit: Option<String>,
    price: Option<Decimal>,
    currency: Option<String>,
    expiry_date: Option<Date>,
    batch_number: Option<String>,
    notes: Option<String>,
    urgency_level: UrgencyLevel,
    expiry_info: Option<String>,
    ai_confidence: f64,
    content_embedding: Option<PgVector>,
}

impl OfferBuilder {
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
            price: None,
            currency: Some("EGP".to_string()),
            expiry_date: None,
            batch_number: None,
            notes: None,
            urgency_level: UrgencyLevel::Normal,
            expiry_info: None,
            ai_confidence: 0.0,
            content_embedding: None,
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
            id: Uuid::new_v4(),
            raw_message_id: self.raw_message_id,
            participant_id: self.participant_id,
            group_id: self.group_id,
            medication: self.medication.clone(),
            medication_raw: self.medication_raw.unwrap_or(self.medication),
            quantity: self.quantity,
            unit: self.unit,
            price: self.price,
            currency: self.currency,
            batch_number: self.batch_number,
            notes: self.notes,
            status: Status::Active,
            urgency_level: self.urgency_level,
            expiry_info: self.expiry_info,
            ai_confidence: self.ai_confidence,
            content_embedding: self.content_embedding,
            master_medication_id: None,
            medication_curated: false,
            confirmed_match_count: 0,
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
        let msg_id = Uuid::new_v4();
        let part_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let offer = OfferBuilder::new(msg_id, "Aspirin 100mg", part_id, group_id).build();

        assert_eq!(offer.medication, "Aspirin 100mg");
        assert_eq!(offer.participant_id, part_id);
        assert_eq!(offer.group_id, group_id);
        assert_eq!(offer.status, Status::Active);
        assert!(!offer.is_urgent());
    }

    #[test]
    fn test_offer_builder_full() {
        let msg_id = Uuid::new_v4();
        let part_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let offer = OfferBuilder::new(msg_id, "Ozempic 1mg", part_id, group_id)
            .quantity(5.0)
            .price(1500.0)
            .unit("boxes")
            .urgent()
            .ai_confidence(0.95)
            .notes("Original packaging")
            .build();

        assert_eq!(offer.medication, "Ozempic 1mg");
        assert_eq!(offer.participant_id, part_id);
        assert!(offer.quantity.is_some());
        assert!(offer.price.is_some());
        assert_eq!(offer.unit, Some("boxes".to_string()));
        assert!(offer.is_urgent());
        assert_eq!(offer.urgency_level, UrgencyLevel::Urgent);
        assert!((offer.ai_confidence - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_offer_is_urgent() {
        let normal = Model::default();
        assert!(!normal.is_urgent());

        let msg_id = Uuid::new_v4();
        let part_id = Uuid::new_v4();
        let group_id = Uuid::new_v4();
        let urgent = OfferBuilder::new(msg_id, "med", part_id, group_id)
            .urgency_level(UrgencyLevel::Critical)
            .build();
        assert!(urgent.is_urgent());
    }
}
