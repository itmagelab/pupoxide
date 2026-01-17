use crate::domain::{
    Transaction,
    error::Result,
};
use std::fs;
use std::path::PathBuf;

pub struct StateStore {
    root: PathBuf,
}

impl StateStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn save_transaction(&self, transaction: &Transaction) -> Result<()> {
        let path = self.get_transaction_path(&transaction.id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("Failed to create state dir: {}", e))?;
        }
        let json = serde_json::to_string_pretty(transaction).map_err(|e| {
            anyhow::anyhow!("Failed to serialize transaction: {}", e)
        })?;
        fs::write(path, json)
            .map_err(|e| anyhow::anyhow!("Failed to write transaction: {}", e))?;

        // Update 'latest' pointer
        let latest_path = self.root.join("latest_transaction");
        fs::write(latest_path, &transaction.id).map_err(|e| {
            anyhow::anyhow!("Failed to update latest transaction: {}", e)
        })?;

        Ok(())
    }

    pub fn load_transaction(&self, id: &str) -> Result<Transaction> {
        let path = self.get_transaction_path(id);
        let json = fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!("Failed to read transaction {}: {}", id, e)
        })?;
        serde_json::from_str(&json)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize transaction: {}", e))
    }

    pub fn load_latest_transaction(&self) -> Result<Transaction> {
        let latest_path = self.root.join("latest_transaction");
        let id = fs::read_to_string(latest_path)
            .map_err(|e| anyhow::anyhow!("No previous transaction found: {}", e))?;
        self.load_transaction(id.trim())
    }

    fn get_transaction_path(&self, id: &str) -> PathBuf {
        self.root.join("transactions").join(format!("{}.json", id))
    }
}
