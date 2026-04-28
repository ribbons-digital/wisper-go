use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use thiserror::Error;

use crate::privacy::PrivacyPolicy;

const PRIVACY_POLICY_KEY: &str = "privacy_policy";
const MIGRATION: &str =
    include_str!("../../../apps/desktop/src-tauri/migrations/0001_initial.sql");

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("system clock is before unix epoch")]
    Clock,
}

pub struct LocalStore {
    conn: Connection,
}

impl LocalStore {
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Ok(Self {
            conn: Connection::open_in_memory()?,
        })
    }

    pub fn migrate(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(MIGRATION)?;
        Ok(())
    }

    pub fn save_privacy_policy(&self, policy: &PrivacyPolicy) -> Result<(), StoreError> {
        let value = serde_json::to_string(policy)?;
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![PRIVACY_POLICY_KEY, value, now()?],
        )?;
        Ok(())
    }

    pub fn load_privacy_policy(&self) -> Result<PrivacyPolicy, StoreError> {
        let value: String = self.conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![PRIVACY_POLICY_KEY],
            |row| row.get(0),
        )?;
        Ok(serde_json::from_str(&value)?)
    }

    pub fn insert_history(&self, text: &str, source: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO history (text, source, created_at) VALUES (?1, ?2, ?3)",
            params![text, source, now()?],
        )?;
        Ok(())
    }

    pub fn history_count(&self) -> Result<i64, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?)
    }
}

fn now() -> Result<i64, StoreError> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreError::Clock)?
        .as_secs() as i64)
}
