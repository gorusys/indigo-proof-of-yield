//! Indigo Protocol–specific parsers and event reconstruction.

pub(crate) mod events;
mod indy_staking;
mod protocol_config;
mod rob;
mod stability_pool;

pub use events::{Event, EventKind, IndigoEvents};
pub use indy_staking::reconstruct_indy_staking_events;
pub use protocol_config::IndigoV2Config;
pub use rob::reconstruct_rob_events;
pub use stability_pool::reconstruct_stability_pool_events;

use crate::chain::fetch::{KoiosAccountTx, KoiosTxUtxos};
use time::OffsetDateTime;

/// Build full IndigoEvents from account txs and a lookup for tx UTxOs.
/// Pass optional Indigo V2 (mainnet) config for accurate parsing (script/datum/policy IDs); when None or empty, uses heuristic mode.
pub fn reconstruct_all_events(
    account_txs: &[KoiosAccountTx],
    get_tx_utxos: impl Fn(&str) -> Option<KoiosTxUtxos>,
    now: OffsetDateTime,
    config: Option<&IndigoV2Config>,
) -> IndigoEvents {
    let default_config = IndigoV2Config::default();
    let config = config.unwrap_or(&default_config);
    let sp = reconstruct_stability_pool_events(account_txs, &get_tx_utxos, now, config);
    let rob = reconstruct_rob_events(account_txs, &get_tx_utxos, now, config);
    let indy = reconstruct_indy_staking_events(account_txs, &get_tx_utxos, now, config);
    let mut events = IndigoEvents {
        stability_pool: sp,
        rob,
        indy_staking: indy,
        other: vec![],
    };
    events.sort_by_slot_then_tx();
    events
}
