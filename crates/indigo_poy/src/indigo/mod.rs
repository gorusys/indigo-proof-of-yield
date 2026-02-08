//! Indigo Protocol–specific parsers and event reconstruction.

pub(crate) mod events;
mod indy_staking;
mod rob;
mod stability_pool;

pub use events::{Event, EventKind, IndigoEvents};
pub use indy_staking::reconstruct_indy_staking_events;
pub use rob::reconstruct_rob_events;
pub use stability_pool::reconstruct_stability_pool_events;

use crate::chain::fetch::{KoiosAccountTx, KoiosTxUtxos};
use time::OffsetDateTime;

/// Build full IndigoEvents from account txs and a lookup for tx UTxOs.
pub fn reconstruct_all_events(
    account_txs: &[KoiosAccountTx],
    get_tx_utxos: impl Fn(&str) -> Option<KoiosTxUtxos>,
    now: OffsetDateTime,
) -> IndigoEvents {
    let sp = reconstruct_stability_pool_events(account_txs, &get_tx_utxos, now);
    let rob = reconstruct_rob_events(account_txs, &get_tx_utxos, now);
    let indy = reconstruct_indy_staking_events(account_txs, &get_tx_utxos, now);
    let mut events = IndigoEvents {
        stability_pool: sp,
        rob,
        indy_staking: indy,
        other: vec![],
    };
    events.sort_by_slot_then_tx();
    events
}
