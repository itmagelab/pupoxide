use serde::{Deserialize, Serialize};
use validator::Validate;

/// Bootstrap request pending approval from admin
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct BootstrapRequest {
    /// Node ID requesting to bootstrap
    #[validate(length(min = 1, max = 255))]
    pub node_id: String,
    /// PEM-encoded Certificate Signing Request (for compatibility, can be empty)
    pub csr: String,
    /// Unix timestamp when request was created
    pub requested_at: i64,
    /// Status of the request: "pending", "approved", "rejected"
    pub status: String,
    /// PEM-encoded self-signed certificate (contains the public key)
    #[serde(default)]
    pub certificate: Option<String>,
}

impl BootstrapRequest {
    pub fn is_pending(&self) -> bool {
        self.status == "pending"
    }

    pub fn is_approved(&self) -> bool {
        self.status == "approved"
    }

    pub fn approve(&mut self) {
        self.status = "approved".to_string();
    }

    pub fn reject(&mut self) {
        self.status = "rejected".to_string();
    }
}

/// Bootstrap request response (sent to agent)
#[derive(Debug, Serialize, Deserialize)]
pub struct BootstrapResponse {
    /// Status: "pending", "approved", or "rejected"
    pub status: String,
    /// Message for the agent
    pub message: String,
    /// Signed certificate (only if approved)
    pub certificate: Option<String>,
    /// CA certificate (only if approved)
    pub ca_certificate: Option<String>,
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
    /// Unix timestamp of registration/approval
    pub approved_at: i64,
    /// Last successful contact with master
    pub last_seen: Option<i64>,
    /// Is agent currently active/trusted
    pub is_active: bool,
}

/// Request listing (for admin UI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapRequestMetadata {
    pub node_id: String,
    pub status: String,
    pub requested_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_request_status() {
        let mut req = BootstrapRequest {
            node_id: "agent-01".to_string(),
            csr: "test_csr".to_string(),
            requested_at: 1234567890,
            status: "pending".to_string(),
            certificate: None,
        };

        assert!(req.is_pending());
        assert!(!req.is_approved());

        req.approve();
        assert!(req.is_approved());
        assert!(!req.is_pending());
    }
}
