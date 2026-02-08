//! Koios (or alternate) API client with rate limiting and retries.

use crate::chain::cache::Cache;
use crate::chain::normalize::{normalize_slot_time, NormalizeError};
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use thiserror::Error;
use time::OffsetDateTime;
use tracing::{debug, info, warn};

const DEFAULT_KOIOS_URL: &str = "https://api.koios.rest/api/v1";
const RATE_LIMIT_MS: u64 = 200;
const MAX_RETRIES: u32 = 3;
const RETRY_BACKOFF_MS: u64 = 500;

#[derive(Clone, Debug)]
pub struct FetchConfig {
    pub base_url: String,
    pub rate_limit_ms: u64,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    pub offline: bool,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_KOIOS_URL.to_string(),
            rate_limit_ms: RATE_LIMIT_MS,
            max_retries: MAX_RETRIES,
            retry_backoff_ms: RETRY_BACKOFF_MS,
            offline: false,
        }
    }
}

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("request: {0}")]
    Request(#[from] reqwest::Error),
    #[error("cache: {0}")]
    Cache(#[from] crate::chain::cache::CacheError),
    #[error("normalize: {0}")]
    Normalize(#[from] NormalizeError),
    #[error("api error: status {0} body {1}")]
    Api(u16, String),
    #[error("offline mode: no cached data for key")]
    OfflineMiss,
}

#[derive(Clone, Deserialize)]
pub struct KoiosAccountTx {
    pub tx_hash: String,
    pub block_height: Option<u64>,
    pub block_time: Option<i64>,
    pub epoch_no: Option<u64>,
    pub slot_no: Option<u64>,
}

#[derive(Clone, Deserialize)]
pub struct KoiosUtxo {
    pub tx_hash: String,
    pub tx_index: u32,
    pub value: String,
    pub datum_hash: Option<String>,
    pub asset_list: Option<Vec<KoiosAsset>>,
}

#[derive(Clone, Deserialize)]
pub struct KoiosAsset {
    pub policy_id: String,
    pub asset_name: String,
    pub quantity: String,
}

#[derive(Clone, Deserialize)]
pub struct KoiosTxUtxos {
    pub inputs: Option<Vec<KoiosUtxo>>,
    pub outputs: Option<Vec<KoiosUtxo>>,
}

/// Fetcher with rate limiting and optional SQLite cache.
pub struct Fetcher {
    config: FetchConfig,
    client: Option<reqwest::Client>,
    cache: Option<Cache>,
    last_request: std::sync::Mutex<Option<OffsetDateTime>>,
    request_count: AtomicU64,
}

impl Fetcher {
    pub fn new(config: FetchConfig, cache: Option<Cache>) -> Result<Self, FetchError> {
        let client = if config.offline {
            None
        } else {
            Some(
                reqwest::Client::builder()
                    .use_rustls_tls()
                    .timeout(Duration::from_secs(30))
                    .build()?,
            )
        };
        Ok(Self {
            config,
            client,
            cache,
            last_request: std::sync::Mutex::new(None),
            request_count: AtomicU64::new(0),
        })
    }

    async fn rate_limit(&self) {
        let sleep_ms = {
            let last = self.last_request.lock().unwrap();
            let prev = *last;
            drop(last);
            if let Some(prev) = prev {
                let elapsed = (OffsetDateTime::now_utc() - prev).whole_milliseconds();
                let need_i: i128 = self.config.rate_limit_ms as i128;
                if elapsed < need_i {
                    (need_i - elapsed).max(0) as u64
                } else {
                    0
                }
            } else {
                0
            }
        };
        if sleep_ms > 0 {
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }
        *self.last_request.lock().unwrap() = Some(OffsetDateTime::now_utc());
    }

    async fn get_json(&self, path: &str, cache_key: &str) -> Result<String, FetchError> {
        self.request_json(path, cache_key, None).await
    }

    async fn request_json(
        &self,
        path: &str,
        cache_key: &str,
        post_body: Option<serde_json::Value>,
    ) -> Result<String, FetchError> {
        if let Some(cache) = &self.cache {
            if let Some(cached) = cache.get_json(cache_key)? {
                debug!(key = %cache_key, "cache hit");
                return Ok(cached);
            }
            if self.config.offline {
                return Err(FetchError::OfflineMiss);
            }
        }

        let client = self.client.as_ref().ok_or(FetchError::OfflineMiss)?;
        self.rate_limit().await;

        let url = format!("{}{}", self.config.base_url.trim_end_matches('/'), path);
        let mut last_err = None;
        for attempt in 0..=self.config.max_retries {
            let res = if let Some(body) = &post_body {
                client.post(&url).json(body).send().await
            } else {
                client.get(&url).send().await
            };
            match res {
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    if !status.is_success() {
                        last_err = Some(FetchError::Api(status.as_u16(), body));
                        if attempt < self.config.max_retries {
                            let ms = self.config.retry_backoff_ms * (1 << attempt);
                            tokio::time::sleep(Duration::from_millis(ms)).await;
                        }
                        continue;
                    }
                    self.request_count.fetch_add(1, Ordering::Relaxed);
                    if let Some(cache) = &self.cache {
                        let _ = cache.set_json(cache_key, &body);
                    }
                    return Ok(body);
                }
                Err(e) => {
                    last_err = Some(FetchError::Request(e));
                    if attempt < self.config.max_retries {
                        let ms = self.config.retry_backoff_ms * (1 << attempt);
                        warn!(attempt, ms, "retry after error");
                        tokio::time::sleep(Duration::from_millis(ms)).await;
                    }
                }
            }
        }
        Err(last_err.unwrap_or(FetchError::Api(0, "unknown".to_string())))
    }

    /// Fetch account transactions in range. from_slot and to_slot are optional (slot numbers).
    pub async fn account_txs(
        &self,
        address: &str,
        from_slot_or_time: Option<&str>,
        to_slot_or_time: Option<&str>,
    ) -> Result<Vec<KoiosAccountTx>, FetchError> {
        let from_parsed = from_slot_or_time.map(normalize_slot_time).transpose()?;
        let to_parsed = to_slot_or_time.map(normalize_slot_time).transpose()?;
        let from_slot = from_parsed.and_then(|(s, _)| s);
        let to_slot = to_parsed.and_then(|(s, _)| s);

        let req = serde_json::json!({
            "address": address,
            "from": from_slot,
            "to": to_slot
        });
        let norm = serde_json::to_string(&req)
            .map_err(|_| FetchError::Api(0, "serialize request".to_string()))?;
        let cache_key = Cache::key_for(&norm);

        let path = "/account_txs";
        let post_body = serde_json::json!({ "_addresses": [address] });
        let body = self.request_json(path, &cache_key, Some(post_body)).await?;
        let parsed: Vec<KoiosAccountTx> = serde_json::from_str(&body).unwrap_or_default();
        info!(count = parsed.len(), "account_txs");
        Ok(parsed)
    }

    /// Fetch UTxOs at address (current).
    pub async fn address_utxos(&self, address: &str) -> Result<Vec<KoiosUtxo>, FetchError> {
        let req = serde_json::json!({ "address": address });
        let norm =
            serde_json::to_string(&req).map_err(|_| FetchError::Api(0, "serialize".to_string()))?;
        let cache_key = Cache::key_for(&norm);
        let path = format!("/address_utxos?_address={}", urlencoding::encode(address));
        let body = self.get_json(&path, &cache_key).await?;
        let parsed: Vec<KoiosUtxo> = serde_json::from_str(&body).unwrap_or_default();
        Ok(parsed)
    }

    /// Fetch tx UTxOs (inputs/outputs) for a tx hash.
    pub async fn tx_utxos(&self, tx_hash: &str) -> Result<KoiosTxUtxos, FetchError> {
        let req = serde_json::json!({ "tx_hash": tx_hash });
        let norm =
            serde_json::to_string(&req).map_err(|_| FetchError::Api(0, "serialize".to_string()))?;
        let cache_key = Cache::key_for(&norm);
        let path = format!("/tx_utxos?_tx_hash={}", urlencoding::encode(tx_hash));
        let body = self.get_json(&path, &cache_key).await?;
        serde_json::from_str(&body)
            .map_err(|e| FetchError::Api(0, format!("parse tx_utxos: {}", e)))
    }

    pub fn request_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }
}
