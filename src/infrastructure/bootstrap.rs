use crate::domain::bootstrap::{BootstrapToken, RegisteredAgent};
use anyhow::{anyhow, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// In-memory bootstrap token manager
/// In production, this would be backed by a database
pub struct BootstrapTokenManager {
    tokens: Arc<RwLock<HashMap<String, BootstrapToken>>>,
}

impl BootstrapTokenManager {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate a new bootstrap token
    pub async fn generate_token(
        &self,
        node_id: &str,
        ttl_seconds: i64,
    ) -> Result<String> {
        let token_str = Uuid::new_v4().to_string();
        let _now = Utc::now().timestamp();

        let token = BootstrapToken {
            token: token_str.clone(),
            node_id: node_id.to_string(),
            issued_at: _now,
            expires_at: _now + ttl_seconds,
            used_at: None,
        };

        let mut tokens = self.tokens.write().await;
        tokens.insert(token_str.clone(), token);

        info!(node_id = node_id, "Generated new bootstrap token");
        Ok(token_str)
    }

    /// Verify and consume a bootstrap token
    pub async fn verify_token(&self, token_str: &str) -> Result<String> {
        let mut tokens = self.tokens.write().await;

        let token = tokens
            .get(token_str)
            .ok_or_else(|| anyhow!("Token not found"))?;

        if !token.is_valid() {
            return Err(anyhow!("Token is invalid or expired"));
        }

        let node_id = token.node_id.clone();

        // Mark token as used
        if let Some(token) = tokens.get_mut(token_str) {
            token.used_at = Some(Utc::now().timestamp());
        }

        info!(node_id = %node_id, "Token verified and consumed");
        Ok(node_id)
    }

    /// Clean up expired tokens
    pub async fn cleanup_expired(&self) {
        let mut tokens = self.tokens.write().await;

        let before_count = tokens.len();
        tokens.retain(|_, token| !token.is_expired());
        let after_count = tokens.len();

        if before_count != after_count {
            debug!(removed = before_count - after_count, "Cleaned up expired tokens");
        }
    }
}

impl Default for BootstrapTokenManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Agent registry (tracks registered agents)
/// In production, this would be backed by a database
pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<String, RegisteredAgent>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new agent
    pub async fn register(
        &self,
        node_id: &str,
        cert_cn: &str,
        certificate_pem: String,
    ) -> Result<()> {
        let agent = RegisteredAgent {
            node_id: node_id.to_string(),
            cert_cn: cert_cn.to_string(),
            certificate_pem,
            registered_at: Utc::now().timestamp(),
            last_seen: None,
            is_active: true,
        };

        let mut agents = self.agents.write().await;
        agents.insert(node_id.to_string(), agent);

        info!(node_id = node_id, "Agent registered");
        Ok(())
    }

    /// Verify if an agent is registered and active
    pub async fn is_registered(&self, node_id: &str) -> Result<bool> {
        let agents = self.agents.read().await;
        let is_registered = agents
            .get(node_id)
            .map(|agent| agent.is_active)
            .unwrap_or(false);

        Ok(is_registered)
    }

    /// Update last_seen timestamp for an agent
    pub async fn update_last_seen(&self, node_id: &str) -> Result<()> {
        let mut agents = self.agents.write().await;

        if let Some(agent) = agents.get_mut(node_id) {
            agent.last_seen = Some(Utc::now().timestamp());
            debug!(node_id = node_id, "Updated last_seen");
            Ok(())
        } else {
            Err(anyhow!("Agent {} not found", node_id))
        }
    }

    /// Revoke an agent (mark as inactive)
    pub async fn revoke(&self, node_id: &str) -> Result<()> {
        let mut agents = self.agents.write().await;

        if let Some(agent) = agents.get_mut(node_id) {
            agent.is_active = false;
            info!(node_id = node_id, "Agent revoked");
            Ok(())
        } else {
            Err(anyhow!("Agent {} not found", node_id))
        }
    }

    /// Get agent info
    pub async fn get_agent(&self, node_id: &str) -> Result<RegisteredAgent> {
        let agents = self.agents.read().await;
        agents
            .get(node_id)
            .cloned()
            .ok_or_else(|| anyhow!("Agent {} not found", node_id))
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_token() {
        let manager = BootstrapTokenManager::new();
        let token = manager
            .generate_token("agent-01", 3600)
            .await
            .expect("Failed to generate token");
        assert!(!token.is_empty());
    }

    #[tokio::test]
    async fn test_verify_token() {
        let manager = BootstrapTokenManager::new();
        let token = manager
            .generate_token("agent-01", 3600)
            .await
            .expect("Failed to generate token");
        let node_id = manager
            .verify_token(&token)
            .await
            .expect("Failed to verify token");
        assert_eq!(node_id, "agent-01");

        // Token should not be valid again
        let result = manager.verify_token(&token).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_register_agent() {
        let registry = AgentRegistry::new();
        registry
            .register("agent-01", "agent-01", "cert_pem".to_string())
            .await
            .expect("Failed to register agent");

        let is_registered = registry
            .is_registered("agent-01")
            .await
            .expect("Failed to check registration");
        assert!(is_registered);
    }

    #[tokio::test]
    async fn test_revoke_agent() {
        let registry = AgentRegistry::new();
        registry
            .register("agent-01", "agent-01", "cert_pem".to_string())
            .await
            .expect("Failed to register agent");

        registry
            .revoke("agent-01")
            .await
            .expect("Failed to revoke agent");

        let is_registered = registry
            .is_registered("agent-01")
            .await
            .expect("Failed to check registration");
        assert!(!is_registered);
    }
}
