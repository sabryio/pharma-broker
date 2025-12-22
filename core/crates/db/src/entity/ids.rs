//! Strongly-typed ID newtypes for type-safe entity references
//!
//! Using newtypes for IDs prevents accidentally mixing up different entity IDs
//! at compile time, improving code safety and self-documentation.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

// =============================================================================
// ID Newtype Macro
// =============================================================================

/// Macro to generate ID newtypes with common functionality
macro_rules! define_entity_id {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Create a new random ID
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

            /// Create from an existing string value
            pub fn from_string(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            /// Get the inner string value
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consume and return the inner string
            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = std::convert::Infallible;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(s.to_string()))
            }
        }

        // SeaORM compatibility
        impl From<$name> for sea_orm::Value {
            fn from(id: $name) -> Self {
                sea_orm::Value::String(Some(Box::new(id.0)))
            }
        }

        impl sea_orm::TryGetable for $name {
            fn try_get_by<I: sea_orm::ColIdx>(
                res: &sea_orm::QueryResult,
                index: I,
            ) -> Result<Self, sea_orm::TryGetError> {
                let val: String = res.try_get_by(index)?;
                Ok(Self(val))
            }
        }

        impl sea_orm::sea_query::ValueType for $name {
            fn try_from(v: sea_orm::Value) -> Result<Self, sea_orm::sea_query::ValueTypeErr> {
                match v {
                    sea_orm::Value::String(Some(s)) => Ok(Self(*s)),
                    _ => Err(sea_orm::sea_query::ValueTypeErr),
                }
            }

            fn type_name() -> String {
                stringify!($name).to_string()
            }

            fn array_type() -> sea_orm::sea_query::ArrayType {
                sea_orm::sea_query::ArrayType::String
            }

            fn column_type() -> sea_orm::sea_query::ColumnType {
                sea_orm::sea_query::ColumnType::String(sea_orm::sea_query::StringLen::None)
            }
        }

        impl sea_orm::sea_query::Nullable for $name {
            fn null() -> sea_orm::Value {
                sea_orm::Value::String(None)
            }
        }
    };
}

// =============================================================================
// Entity ID Types
// =============================================================================

define_entity_id!(
    /// Unique identifier for an Offer entity
    OfferId
);

define_entity_id!(
    /// Unique identifier for a Request entity
    RequestId
);

define_entity_id!(
    /// Unique identifier for a Match entity
    MatchId
);

define_entity_id!(
    /// Unique identifier for a RawMessage entity
    RawMessageId
);

define_entity_id!(
    /// Unique identifier for a MedicationMapping entity
    MedicationMappingId
);

// =============================================================================
// WhatsApp-specific IDs
// =============================================================================

/// WhatsApp Group JID (Jabber ID)
///
/// Format: `<number>@g.us` for groups
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GroupJid(String);

impl GroupJid {
    /// Create from a JID string
    pub fn new(jid: impl Into<String>) -> Self {
        Self(jid.into())
    }

    /// Get the inner JID string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if this is a valid group JID
    pub fn is_valid_group(&self) -> bool {
        self.0.ends_with("@g.us")
    }

    /// Extract the group number from the JID
    pub fn group_number(&self) -> Option<&str> {
        self.0.strip_suffix("@g.us")
    }
}

impl fmt::Display for GroupJid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for GroupJid {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for GroupJid {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for GroupJid {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// WhatsApp User JID (Jabber ID)
///
/// Format: `<phone>@s.whatsapp.net` for users
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserJid(String);

impl UserJid {
    /// Create from a JID string
    pub fn new(jid: impl Into<String>) -> Self {
        Self(jid.into())
    }

    /// Get the inner JID string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if this is a valid user JID
    pub fn is_valid_user(&self) -> bool {
        self.0.ends_with("@s.whatsapp.net")
    }

    /// Extract the phone number from the JID
    pub fn phone_number(&self) -> Option<&str> {
        self.0.strip_suffix("@s.whatsapp.net")
    }
}

impl fmt::Display for UserJid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for UserJid {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for UserJid {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offer_id_creation() {
        let id = OfferId::new();
        assert!(!id.as_str().is_empty());

        let id2 = OfferId::from_string("test-id");
        assert_eq!(id2.as_str(), "test-id");
    }

    #[test]
    fn test_offer_id_display() {
        let id = OfferId::from_string("abc-123");
        assert_eq!(format!("{}", id), "abc-123");
    }

    #[test]
    fn test_offer_id_from_string() {
        let id: OfferId = "test".into();
        assert_eq!(id.as_str(), "test");

        let id2: OfferId = String::from("test2").into();
        assert_eq!(id2.as_str(), "test2");
    }

    #[test]
    fn test_offer_id_into_string() {
        let id = OfferId::from_string("test");
        let s: String = id.into();
        assert_eq!(s, "test");
    }

    #[test]
    fn test_group_jid_validation() {
        let valid = GroupJid::new("123456789@g.us");
        assert!(valid.is_valid_group());
        assert_eq!(valid.group_number(), Some("123456789"));

        let invalid = GroupJid::new("123456789@s.whatsapp.net");
        assert!(!invalid.is_valid_group());
        assert_eq!(invalid.group_number(), None);
    }

    #[test]
    fn test_user_jid_validation() {
        let valid = UserJid::new("201234567890@s.whatsapp.net");
        assert!(valid.is_valid_user());
        assert_eq!(valid.phone_number(), Some("201234567890"));

        let invalid = UserJid::new("123456789@g.us");
        assert!(!invalid.is_valid_user());
        assert_eq!(invalid.phone_number(), None);
    }

    #[test]
    fn test_different_id_types_not_mixable() {
        // This test documents that different ID types are distinct
        // The following would NOT compile:
        // let offer_id: OfferId = RequestId::new().into();

        let offer_id = OfferId::new();
        let request_id = RequestId::new();

        // They can both be converted to String
        let _s1: String = offer_id.into();
        let _s2: String = request_id.into();
    }

    #[test]
    fn test_id_serialization() {
        let id = OfferId::from_string("test-123");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"test-123\"");

        let parsed: OfferId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_str(), "test-123");
    }
}
