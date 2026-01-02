//! Audit action and entity types for audit logging

use serde::{Deserialize, Serialize};

/// Types of auditable actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    // Match actions
    MatchCreated,
    MatchConfirmed,
    MatchRejected,
    MatchAutoConfirmed,
    MatchReAudited,
    MatchRecalculated,

    // Offer/Request actions
    OfferCreated,
    RequestCreated,
    OfferExpired,
    RequestExpired,
    ItemReclassified,
    ItemReparsed,

    // Weight/Config actions
    WeightsUpdated,
    WeightsRollback,
    ABTestCreated,
    ABTestEnded,

    // Review Queue actions
    ReviewQueued,
    ReviewApproved,
    ReviewRejected,
    ReviewSkipped,

    // System actions
    SystemStartup,
    ConfigChanged,

    // Generic
    Custom(String),
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MatchCreated => write!(f, "match_created"),
            Self::MatchConfirmed => write!(f, "match_confirmed"),
            Self::MatchRejected => write!(f, "match_rejected"),
            Self::MatchAutoConfirmed => write!(f, "match_auto_confirmed"),
            Self::MatchReAudited => write!(f, "match_re_audited"),
            Self::MatchRecalculated => write!(f, "match_recalculated"),
            Self::OfferCreated => write!(f, "offer_created"),
            Self::RequestCreated => write!(f, "request_created"),
            Self::OfferExpired => write!(f, "offer_expired"),
            Self::RequestExpired => write!(f, "request_expired"),
            Self::ItemReclassified => write!(f, "item_reclassified"),
            Self::ItemReparsed => write!(f, "item_reparsed"),
            Self::WeightsUpdated => write!(f, "weights_updated"),
            Self::WeightsRollback => write!(f, "weights_rollback"),
            Self::ABTestCreated => write!(f, "ab_test_created"),
            Self::ABTestEnded => write!(f, "ab_test_ended"),
            Self::ReviewQueued => write!(f, "review_queued"),
            Self::ReviewApproved => write!(f, "review_approved"),
            Self::ReviewRejected => write!(f, "review_rejected"),
            Self::ReviewSkipped => write!(f, "review_skipped"),
            Self::SystemStartup => write!(f, "system_startup"),
            Self::ConfigChanged => write!(f, "config_changed"),
            Self::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Types of entities that can be audited
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Match,
    Offer,
    Request,
    Weights,
    ABTest,
    ReviewQueue,
    Group,
    System,
    Custom(String),
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Match => write!(f, "match"),
            Self::Offer => write!(f, "offer"),
            Self::Request => write!(f, "request"),
            Self::Weights => write!(f, "weights"),
            Self::ABTest => write!(f, "ab_test"),
            Self::ReviewQueue => write!(f, "review_queue"),
            Self::Group => write!(f, "group"),
            Self::System => write!(f, "system"),
            Self::Custom(s) => write!(f, "{}", s),
        }
    }
}
