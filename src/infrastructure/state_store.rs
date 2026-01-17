use crate::domain::{
    Transaction,
    error::Result,
};
use anyhow::Context;
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
                .context("Failed to create state dir")?;
        }
        let json = serde_json::to_string_pretty(transaction)
            .context("Failed to serialize transaction")?;
        fs::write(path, json)
            .context("Failed to write transaction")?;

        // Update 'latest' pointer
        let latest_path = self.root.join("latest_transaction");
        fs::write(latest_path, &transaction.id)
            .context("Failed to update latest transaction")?;

        Ok(())
    }

    pub fn load_transaction(&self, id: &str) -> Result<Transaction> {
        let path = self.get_transaction_path(id);
        let json = fs::read_to_string(path)
            .context(format!("Failed to read transaction {}", id))?;
        serde_json::from_str(&json)
            .context("Failed to deserialize transaction")
    }

    pub fn load_latest_transaction(&self) -> Result<Transaction> {
        let latest_path = self.root.join("latest_transaction");
        let id = fs::read_to_string(latest_path)
            .context("No previous transaction found")?;
        self.load_transaction(id.trim())
    }

    fn get_transaction_path(&self, id: &str) -> PathBuf {
        self.root.join("transactions").join(format!("{}.json", id))
    }
}
