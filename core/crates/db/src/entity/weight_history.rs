//! WeightHistory entity - Historical weight configurations

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "weight_history")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub medication_weight: f64,
    pub dosage_weight: f64,
    pub quantity_weight: f64,
    pub price_weight: f64,
    pub recency_weight: f64,
    pub source: String,
    pub sample_count: i32,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
