//! JID (Jabber ID) validation for WhatsApp identifiers
//!
//! Validates WhatsApp JID formats:
//! - Individual: {phone}@s.whatsapp.net
//! - Group: {id}@g.us
//! - LID: {id}@lid

use thiserror::Error;

/// Valid WhatsApp JID server suffixes
pub const VALID_JID_SERVERS: &[&str] = &["s.whatsapp.net", "g.us", "lid"];

/// Error type for JID validation
#[derive(Debug, Error, PartialEq)]
pub enum JidError {
    #[error("JID cannot be empty")]
    Empty,

    #[error("JID must contain exactly one @ symbol")]
    InvalidFormat,

    #[error("JID identifier part cannot be empty")]
    EmptyIdentifier,

    #[error("JID server part cannot be empty")]
    EmptyServer,

    #[error("Invalid server '{0}', must be one of: s.whatsapp.net, g.us, lid")]
    InvalidServer(String),
}

/// Validates a WhatsApp JID string
///
/// # Arguments
/// * `jid` - The JID string to validate
///
/// # Returns
/// * `Ok(())` if the JID is valid
/// * `Err(JidError)` if the JID is invalid
///
/// # Examples
/// ```
/// use pharma_core::domain::jid::validate_jid;
///
/// assert!(validate_jid("201234567890@s.whatsapp.net").is_ok());
/// assert!(validate_jid("120363123456789012@g.us").is_ok());
/// assert!(validate_jid("abc123@lid").is_ok());
/// assert!(validate_jid("invalid").is_err());
/// ```
pub fn validate_jid(jid: &str) -> Result<(), JidError> {
    if jid.is_empty() {
        return Err(JidError::Empty);
    }

    let parts: Vec<&str> = jid.split('@').collect();
    if parts.len() != 2 {
        return Err(JidError::InvalidFormat);
    }

    let identifier = parts[0];
    let server = parts[1];

    if identifier.is_empty() {
        return Err(JidError::EmptyIdentifier);
    }

    if server.is_empty() {
        return Err(JidError::EmptyServer);
    }

    if !VALID_JID_SERVERS.contains(&server) {
        return Err(JidError::InvalidServer(server.to_string()));
    }

    Ok(())
}

/// Returns true if the JID string is valid
pub fn is_valid_jid(jid: &str) -> bool {
    validate_jid(jid).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Feature: send-message, Property 3: Invalid JID Format Rejected
    // Validates: Requirements 1.3

    #[test]
    fn test_valid_individual_jid() {
        assert!(validate_jid("201234567890@s.whatsapp.net").is_ok());
        assert!(validate_jid("1@s.whatsapp.net").is_ok());
    }

    #[test]
    fn test_valid_group_jid() {
        assert!(validate_jid("120363123456789012@g.us").is_ok());
    }

    #[test]
    fn test_valid_lid_jid() {
        assert!(validate_jid("abc123@lid").is_ok());
    }

    #[test]
    fn test_empty_jid() {
        assert_eq!(validate_jid(""), Err(JidError::Empty));
    }

    #[test]
    fn test_no_at_symbol() {
        assert_eq!(validate_jid("invalid"), Err(JidError::InvalidFormat));
    }

    #[test]
    fn test_multiple_at_symbols() {
        assert_eq!(validate_jid("a@b@c"), Err(JidError::InvalidFormat));
    }

    #[test]
    fn test_empty_identifier() {
        assert_eq!(
            validate_jid("@s.whatsapp.net"),
            Err(JidError::EmptyIdentifier)
        );
    }

    #[test]
    fn test_empty_server() {
        assert_eq!(validate_jid("123@"), Err(JidError::EmptyServer));
    }

    #[test]
    fn test_invalid_server() {
        let result = validate_jid("123@unknown.net");
        assert!(matches!(result, Err(JidError::InvalidServer(_))));
    }

    #[test]
    fn test_is_valid_jid() {
        assert!(is_valid_jid("201234567890@s.whatsapp.net"));
        assert!(!is_valid_jid("invalid"));
    }
}
