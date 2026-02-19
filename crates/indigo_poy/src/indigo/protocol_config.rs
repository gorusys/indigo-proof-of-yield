//! Indigo Protocol V2 (current mainnet) on-chain identifiers for accurate protocol parsing.
//!
//! When these values are set (via config file or env), the parsers restrict
//! Stability Pool / ROB / INDY events to UTxOs that match the official script
//! or policy IDs. When empty, the tool falls back to heuristic detection.
//!
//! Load from: env `INDIGO_V2_CONFIG_PATH`, or `./config/indigo_v2.json`, or `./indigo_v2.json`.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Indigo Protocol V2 (current mainnet) identifiers (script hashes, datum hashes, policy IDs).
/// Paste values from Indigo team / docs; leave empty for heuristic mode.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IndigoV2Config {
    /// Stability Pool: script address(es) or validator hash(es) (hex).
    /// Used to recognize SP script UTxOs when API provides script/address.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stability_pool_script_hashes: Vec<String>,

    /// Stability Pool: datum hash(es) for SP script UTxOs (hex).
    /// When set, only UTxOs with this datum_hash are treated as SP.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stability_pool_datum_hashes: Vec<String>,

    /// iAsset policy IDs (56-char hex). e.g. iBTC, iETH, iUSD, etc.
    /// When set, only assets with these policy IDs are treated as iAssets for SP.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub iasset_policy_ids: Vec<String>,

    /// ROB: script address(es) or validator hash(es) (hex).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rob_script_hashes: Vec<String>,

    /// ROB: datum hash(es) for ROB script UTxOs (hex).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rob_datum_hashes: Vec<String>,

    /// INDY token policy ID (56-char hex). Used to recognize INDY rewards/flows.
    #[serde(default)]
    pub indy_policy_id: Option<String>,
}

impl IndigoV2Config {
    /// Load config from path. Returns default (empty) on error or missing file.
    pub fn load_from_path(path: &Path) -> Self {
        let Ok(content) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&content).unwrap_or_default()
    }

    /// Load config: env INDIGO_V2_CONFIG_PATH, then ./config/indigo_v2.json, then ./indigo_v2.json.
    pub fn load() -> Self {
        if let Ok(path) = std::env::var("INDIGO_V2_CONFIG_PATH") {
            let p = Path::new(&path);
            if p.exists() {
                return Self::load_from_path(p);
            }
        }
        for candidate in [
            Path::new("./config/indigo_v2.json"),
            Path::new("./indigo_v2.json"),
        ] {
            if candidate.exists() {
                return Self::load_from_path(candidate);
            }
        }
        Self::default()
    }

    /// True if we have at least one iAsset policy ID (SP filtering is strict).
    pub fn has_iasset_policy_ids(&self) -> bool {
        !self.iasset_policy_ids.is_empty()
    }

    /// True if we have at least one SP datum hash (SP filtering by datum).
    pub fn has_stability_pool_datum_hashes(&self) -> bool {
        !self.stability_pool_datum_hashes.is_empty()
    }

    /// True if we have at least one ROB datum hash (ROB filtering by datum).
    pub fn has_rob_datum_hashes(&self) -> bool {
        !self.rob_datum_hashes.is_empty()
    }

    /// Normalize for comparison: lowercase hex, no 0x prefix.
    fn norm_hex(s: &str) -> String {
        s.trim().trim_start_matches("0x").to_lowercase()
    }

    /// Check if policy_id is a known iAsset policy.
    pub fn is_known_iasset_policy(&self, policy_id: &str) -> bool {
        if self.iasset_policy_ids.is_empty() {
            return true; // heuristic: accept any
        }
        let n = Self::norm_hex(policy_id);
        self.iasset_policy_ids
            .iter()
            .any(|p| Self::norm_hex(p) == n)
    }

    /// Check if datum_hash matches a known Stability Pool datum.
    pub fn is_stability_pool_datum(&self, datum_hash: Option<&str>) -> bool {
        let Some(d) = datum_hash else {
            return self.stability_pool_datum_hashes.is_empty();
        };
        if self.stability_pool_datum_hashes.is_empty() {
            return true;
        }
        let n = Self::norm_hex(d);
        self.stability_pool_datum_hashes
            .iter()
            .any(|h| Self::norm_hex(h) == n)
    }

    /// Check if datum_hash matches a known ROB datum.
    pub fn is_rob_datum(&self, datum_hash: Option<&str>) -> bool {
        let Some(d) = datum_hash else {
            return self.rob_datum_hashes.is_empty();
        };
        if self.rob_datum_hashes.is_empty() {
            return true;
        }
        let n = Self::norm_hex(d);
        self.rob_datum_hashes.iter().any(|h| Self::norm_hex(h) == n)
    }
}
