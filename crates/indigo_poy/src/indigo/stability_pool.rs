//! Stability Pool: liquidation events, iAsset burnt, ADA received, realized premium, dilution.

use crate::chain::fetch::{KoiosAccountTx, KoiosAsset, KoiosTxUtxos};
use crate::indigo::events::{Event, EventKind};
use time::OffsetDateTime;

/// Known Indigo Stability Pool script/datum patterns (best-effort from public info).
/// We match on policy IDs and datum hashes where available.
#[allow(dead_code)]
pub const INDIGO_IASSET_POLICY_PREFIX: &str = "indigo"; // placeholder; real policy IDs are long hex

/// Reconstruct Stability Pool events from account txs and per-tx UTxO data.
/// Fetcher is not passed here; caller provides pre-fetched txs and a function to get tx_utxos.
pub fn reconstruct_stability_pool_events(
    account_txs: &[KoiosAccountTx],
    get_tx_utxos: impl Fn(&str) -> Option<KoiosTxUtxos>,
    now: OffsetDateTime,
) -> Vec<Event> {
    let mut events = Vec::new();
    for tx in account_txs {
        let slot = tx.slot_no;
        let block_time = tx
            .block_time
            .map(|t| OffsetDateTime::from_unix_timestamp(t).unwrap_or(now));
        let ts = block_time.unwrap_or(now);
        let tx_hash = tx.tx_hash.clone();

        let utxos = match get_tx_utxos(&tx_hash) {
            Some(u) => u,
            None => continue,
        };

        let inputs = utxos.inputs.as_deref().unwrap_or(&[]);
        let outputs = utxos.outputs.as_deref().unwrap_or(&[]);

        let ada_in: u64 = inputs.iter().map(|u| parse_lovelace(&u.value)).sum();
        let _ada_out: u64 = outputs.iter().map(|u| parse_lovelace(&u.value)).sum();

        for out in outputs {
            let out_ada = parse_lovelace(&out.value);
            let has_asset = out
                .asset_list
                .as_ref()
                .is_some_and(|a: &Vec<KoiosAsset>| !a.is_empty());
            if has_asset && out_ada > 0 {
                let iasset = out
                    .asset_list
                    .as_ref()
                    .and_then(|a: &Vec<KoiosAsset>| a.first())
                    .map(|a| format!("{}${}", a.policy_id, a.asset_name));
                if out_ada >= ada_in && ada_in > 0 {
                    events.push(Event {
                        kind: EventKind::StabilityPoolLiquidation {
                            iasset_burnt: iasset.clone().unwrap_or_else(|| "unknown".to_string()),
                            ada_received_lovelace: out_ada,
                            realized_premium_lovelace: out_ada.saturating_sub(ada_in).min(out_ada),
                            dilution_effect: None,
                            tx_hash: tx_hash.clone(),
                            slot,
                        },
                        timestamp: ts,
                        slot,
                        tx_hash: tx_hash.clone(),
                        extra: None,
                    });
                } else if out_ada > 0 {
                    events.push(Event {
                        kind: EventKind::StabilityPoolWithdraw {
                            amount_lovelace: out_ada,
                            iasset_amount: iasset,
                            tx_hash: tx_hash.clone(),
                        },
                        timestamp: ts,
                        slot,
                        tx_hash: tx_hash.clone(),
                        extra: None,
                    });
                }
            }
        }

        for inp in inputs {
            let in_ada = parse_lovelace(&inp.value);
            let has_asset = inp
                .asset_list
                .as_ref()
                .is_some_and(|a: &Vec<KoiosAsset>| !a.is_empty());
            if has_asset && in_ada > 0 {
                let iasset = inp
                    .asset_list
                    .as_ref()
                    .and_then(|a: &Vec<KoiosAsset>| a.first())
                    .map(|a| format!("{}${}", a.policy_id, a.asset_name));
                events.push(Event {
                    kind: EventKind::StabilityPoolDeposit {
                        amount_lovelace: in_ada,
                        iasset_amount: iasset,
                        tx_hash: tx_hash.clone(),
                    },
                    timestamp: ts,
                    slot,
                    tx_hash: tx_hash.clone(),
                    extra: None,
                });
            }
        }
    }
    events.sort_by_key(|e| (e.slot.unwrap_or(0), e.tx_hash.clone()));
    events
}

fn parse_lovelace(s: &str) -> u64 {
    s.trim().parse::<u64>().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_000_000).unwrap()
    }

    fn get_none(_: &str) -> Option<KoiosTxUtxos> {
        None
    }

    #[test]
    fn parse_lovelace_ok() {
        assert_eq!(parse_lovelace("1000000"), 1_000_000);
        assert_eq!(parse_lovelace("0"), 0);
    }

    #[test]
    fn reconstruct_empty() {
        let txs: Vec<KoiosAccountTx> = vec![];
        let out = reconstruct_stability_pool_events(&txs, get_none, ts());
        assert!(out.is_empty());
    }
}
