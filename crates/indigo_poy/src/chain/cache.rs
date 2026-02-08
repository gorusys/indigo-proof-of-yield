//! SQLite cache with content-hash keys for fetched API responses.

use rusqlite::{Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Content-addressed cache for API responses. Key = SHA-256 of request params (normalized).
pub struct Cache {
    conn: Mutex<Connection>,
}

impl Cache {
    /// Open or create cache at `path`. Creates parent dirs if needed.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CacheError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS cache (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                created_utc INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_cache_created ON cache(created_utc);
            "#,
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Compute content-hash key from normalized request identifier (e.g. JSON string).
    pub fn key_for(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Get cached value by key. Returns None if missing.
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut stmt = conn.prepare("SELECT value FROM cache WHERE key = ?1")?;
        let row = stmt
            .query_row([key], |r| r.get::<_, Vec<u8>>(0))
            .optional()?;
        Ok(row)
    }

    /// Insert or replace value for key.
    pub fn set(&self, key: &str, value: &[u8]) -> Result<(), CacheError> {
        let created = time::OffsetDateTime::now_utc().unix_timestamp();
        let conn = self
            .conn
            .lock()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO cache (key, value, created_utc) VALUES (?1, ?2, ?3)",
            rusqlite::params![key, value, created],
        )?;
        Ok(())
    }

    /// Get JSON string from cache; returns None if key missing or invalid UTF-8.
    pub fn get_json(&self, key: &str) -> Result<Option<String>, CacheError> {
        let raw = self.get(key)?;
        Ok(raw.and_then(|b| String::from_utf8(b).ok()))
    }

    /// Cache a JSON string. Key should be from `key_for(normalized_request)`.
    pub fn set_json(&self, key: &str, json: &str) -> Result<(), CacheError> {
        self.set(key, json.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn cache_key_deterministic() {
        let k1 = Cache::key_for(r#"{"addr":"x","from":1}"#);
        let k2 = Cache::key_for(r#"{"addr":"x","from":1}"#);
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 64);
    }

    #[test]
    fn cache_get_set_roundtrip() {
        let tmp = NamedTempFile::new().unwrap();
        let cache = Cache::open(tmp.path()).unwrap();
        let key = Cache::key_for("req1");
        cache.set(&key, b"hello").unwrap();
        assert_eq!(cache.get(&key).unwrap(), Some(b"hello".to_vec()));
        assert!(cache.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn cache_json_roundtrip() {
        let tmp = NamedTempFile::new().unwrap();
        let cache = Cache::open(tmp.path()).unwrap();
        let key = Cache::key_for("req2");
        let json = r#"{"a":1}"#;
        cache.set_json(&key, json).unwrap();
        assert_eq!(cache.get_json(&key).unwrap(), Some(json.to_string()));
    }
}
