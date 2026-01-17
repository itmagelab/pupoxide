use serde::{Deserialize, Serialize};
use validator::Validate;

/// Bootstrap token for agent registration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct BootstrapToken {
    /// The token string (UUID-like, random)
    pub token: String,
    /// Node ID this token is for
    pub node_id: String,
    /// Unix timestamp when token was issued
    pub issued_at: i64,
    /// Unix timestamp when token expires
    pub expires_at: i64,
    /// Unix timestamp when token was used (None if not yet used)
    pub used_at: Option<i64>,
}

impl BootstrapToken {
    /// Check if token is still valid
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.used_at.is_none() && now < self.expires_at
    }

    /// Check if token has expired
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.expires_at
    }
}

/// Registered agent information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredAgent {
    /// Unique node identifier
    pub node_id: String,
    /// Certificate Common Name (should match node_id)
    pub cert_cn: String,
    /// PEM-encoded signed certificate
    pub certificate_pem: String,
    /// Unix timestamp of registration
    pub registered_at: i64,
    /// Last successful contact with master
    pub last_seen: Option<i64>,
    /// Is agent currently active/trusted
    pub is_active: bool,
}

/// Bootstrap request payload (sent by agent)
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct BootstrapRequest {
    /// Node ID requesting bootstrap
    #[validate(length(min = 1, max = 255))]
    pub node_id: String,
    /// PEM-encoded Certificate Signing Request
    #[validate(length(min = 1))]
    pub csr: String,
}

/// Bootstrap response payload (sent by master)
#[derive(Debug, Serialize, Deserialize)]
pub struct BootstrapResponse {
    /// PEM-encoded signed certificate for agent
    pub certificate: String,
    /// PEM-encoded CA certificate (for agent to verify server)
    pub ca_certificate: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_token_validity() {
        let now = chrono::Utc::now().timestamp();
        let token = BootstrapToken {
            token: "test_token".to_string(),
            node_id: "agent-01".to_string(),
            issued_at: now,
            expires_at: now + 3600,
            used_at: None,
        };

        assert!(token.is_valid());
        assert!(!token.is_expired());
    }

    #[test]
    fn test_bootstrap_token_expired() {
        let now = chrono::Utc::now().timestamp();
        let token = BootstrapToken {
            token: "test_token".to_string(),
            node_id: "agent-01".to_string(),
            issued_at: now - 7200,
            expires_at: now - 3600,
            used_at: None,
        };

        assert!(!token.is_valid());
        assert!(token.is_expired());
    }

    #[test]
    fn test_bootstrap_token_used() {
        let now = chrono::Utc::now().timestamp();
        let token = BootstrapToken {
            token: "test_token".to_string(),
            node_id: "agent-01".to_string(),
            issued_at: now,
            expires_at: now + 3600,
            used_at: Some(now + 100),
        };

        assert!(!token.is_valid());
    }
}
