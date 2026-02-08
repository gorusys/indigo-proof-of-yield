//! Unified event type for Stability Pool, ROB, and INDY staking.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    StabilityPoolDeposit {
        amount_lovelace: u64,
        iasset_amount: Option<String>,
        tx_hash: String,
    },
    StabilityPoolWithdraw {
        amount_lovelace: u64,
        iasset_amount: Option<String>,
        tx_hash: String,
    },
    StabilityPoolLiquidation {
        iasset_burnt: String,
        ada_received_lovelace: u64,
        realized_premium_lovelace: u64,
        dilution_effect: Option<String>,
        tx_hash: String,
        slot: Option<u64>,
    },
    RobOrderPlace {
        order_id: Option<String>,
        amount_lovelace: u64,
        tx_hash: String,
        slot: Option<u64>,
    },
    RobOrderFill {
        order_id: Option<String>,
        filled_lovelace: u64,
        premium_pct: Option<f64>,
        reimbursement_pct: Option<f64>,
        tx_hash: String,
        slot: Option<u64>,
    },
    RobCooldown {
        inferred_from_tx: bool,
        tx_hash: String,
    },
    IndyStakingReward {
        amount_lovelace: u64,
        epoch: Option<u64>,
        tx_hash: String,
    },
    IndySpPremium {
        amount_lovelace: u64,
        tx_hash: String,
        slot: Option<u64>,
    },
    OtherFlow {
        description: String,
        amount_lovelace: Option<u64>,
        tx_hash: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub kind: EventKind,
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
    pub slot: Option<u64>,
    pub tx_hash: String,
    pub extra: Option<serde_json::Value>,
}

impl Event {
    pub fn tx_hash(&self) -> &str {
        &self.tx_hash
    }
}

/// Collected Indigo-related events for an address.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IndigoEvents {
    pub stability_pool: Vec<Event>,
    pub rob: Vec<Event>,
    pub indy_staking: Vec<Event>,
    pub other: Vec<Event>,
}

impl IndigoEvents {
    pub fn all_events(&self) -> impl Iterator<Item = &Event> {
        self.stability_pool
            .iter()
            .chain(self.rob.iter())
            .chain(self.indy_staking.iter())
            .chain(self.other.iter())
    }

    pub fn sort_by_slot_then_tx(&mut self) {
        let key = |e: &Event| (e.slot.unwrap_or(0), e.tx_hash.clone());
        self.stability_pool.sort_by_key(key);
        self.rob.sort_by_key(key);
        self.indy_staking.sort_by_key(key);
        self.other.sort_by_key(key);
    }
}
