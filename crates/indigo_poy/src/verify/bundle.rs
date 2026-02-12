//! Evidence bundle and SHA-256 reproducibility hash.

use crate::compute::ComputedMetrics;
use crate::indigo::IndigoEvents;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VerifyError {
    #[error("serialize: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Evidence bundle: inputs + computed outputs for reproducibility.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceBundle {
    pub version: u32,
    pub address: String,
    pub created_utc_rfc3339: String,
    /// Tx hashes used as input (sorted).
    pub tx_hashes: Vec<String>,
    /// UTxO / datum / policy IDs referenced (sorted).
    pub input_refs: Vec<String>,
    /// API responses included by content hash (sorted).
    pub api_response_hashes: Vec<String>,
    pub events: IndigoEvents,
    pub metrics: ComputedMetrics,
    /// Optional: raw fetched payload hashes for offline verification.
    pub fetched_at_slots: Vec<u64>,
}

const BUNDLE_VERSION: u32 = 1;

impl EvidenceBundle {
    pub fn new(
        address: String,
        tx_hashes: Vec<String>,
        input_refs: Vec<String>,
        api_response_hashes: Vec<String>,
        events: IndigoEvents,
        metrics: ComputedMetrics,
        fetched_at_slots: Vec<u64>,
    ) -> Self {
        let created_utc_rfc3339 = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "".to_string());
        Self {
            version: BUNDLE_VERSION,
            address,
            created_utc_rfc3339,
            tx_hashes,
            input_refs,
            api_response_hashes,
            events,
            metrics,
            fetched_at_slots,
        }
    }

    /// Demo bundle for screenshots and Discord pitch (fixed timestamp, deterministic hash).
    pub fn demo() -> Self {
        use crate::compute::{CombinedMetrics, ComputedMetrics, IndyStakingMetrics, RobMetrics, StabilityPoolMetrics};
        let stability_pool = StabilityPoolMetrics {
            total_deposits_lovelace: 50_000_000,
            total_withdrawals_lovelace: 0,
            total_liquidations_ada_received_lovelace: 11_270_000, // ~0.49 ADA × 23
            total_realized_premium_lovelace: 1_093_190,
            net_ada_from_liquidations_lovelace: -38_730_000,
            liquidation_count: 23,
        };
        let rob = RobMetrics {
            total_placed_lovelace: 20_000_000,
            total_filled_lovelace: 8_080_000,
            total_premium_received_lovelace: 80_800,
            avg_premium_pct: Some(1.0),
            fill_count: 4,
        };
        let indy_staking = IndyStakingMetrics::default();
        let combined = CombinedMetrics {
            total_ada_in_lovelace: 50_000_000 + 20_000_000,
            total_ada_out_lovelace: 11_270_000 + 8_080_000,
            net_pnl_lovelace: -50_730_000,
            apr_pct: Some(9.7),
        };
        let metrics = ComputedMetrics {
            stability_pool,
            rob,
            indy_staking,
            combined,
            dilution: None,
        };
        Self {
            version: BUNDLE_VERSION,
            address: "addr1_demo (Indigo Proof-of-Yield sample)".to_string(),
            created_utc_rfc3339: "2026-02-12T10:00:00Z".to_string(),
            tx_hashes: vec!["demo_tx_1".into(), "demo_tx_2".into()],
            input_refs: vec![],
            api_response_hashes: vec![],
            events: IndigoEvents::default(),
            metrics,
            fetched_at_slots: vec![100_000, 100_100],
        }
    }
}

/// Normalize JSON for hashing: sort keys and no whitespace.
pub fn normalize_for_hash(value: &serde_json::Value) -> Result<String, VerifyError> {
    let sorted = sort_json_keys(value);
    Ok(serde_json::to_string(&sorted)?)
}

fn sort_json_keys(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => {
            let mut keys: Vec<_> = m.keys().collect();
            keys.sort();
            let out: std::collections::BTreeMap<String, serde_json::Value> = keys
                .into_iter()
                .map(|k| (k.clone(), sort_json_keys(m.get(k).unwrap())))
                .collect();
            serde_json::Value::Object(serde_json::Map::from_iter(out))
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sort_json_keys).collect())
        }
        other => other.clone(),
    }
}

/// Compute SHA-256 over normalized bundle JSON.
pub fn reproducibility_hash(bundle: &EvidenceBundle) -> Result<String, VerifyError> {
    let json = serde_json::to_value(bundle)?;
    let normalized = normalize_for_hash(&json)?;
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationResult {
    pub bundle_hash: String,
    pub expected_hash: Option<String>,
    pub matches: bool,
}

/// Verify a bundle file against an expected .sha256 file content.
#[allow(dead_code)]
pub fn verify_bundle_hash(
    bundle: &EvidenceBundle,
    expected_hex: &str,
) -> Result<VerificationResult, VerifyError> {
    let bundle_hash = reproducibility_hash(bundle)?;
    let expected = expected_hex.trim().to_lowercase();
    let matches = bundle_hash.to_lowercase() == expected;
    Ok(VerificationResult {
        bundle_hash,
        expected_hash: Some(expected),
        matches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indigo::IndigoEvents;

    #[test]
    fn normalize_deterministic() {
        let a = serde_json::json!({"z":1,"a":2});
        let b = serde_json::json!({"a":2,"z":1});
        let na = normalize_for_hash(&a).unwrap();
        let nb = normalize_for_hash(&b).unwrap();
        assert_eq!(na, nb);
    }

    #[test]
    fn hash_deterministic() {
        let bundle = EvidenceBundle::new(
            "addr1".to_string(),
            vec!["tx1".into()],
            vec![],
            vec![],
            IndigoEvents::default(),
            Default::default(),
            vec![100],
        );
        let h1 = reproducibility_hash(&bundle).unwrap();
        let h2 = reproducibility_hash(&bundle).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }
}
