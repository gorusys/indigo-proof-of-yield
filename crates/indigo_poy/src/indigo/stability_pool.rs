//! Stability Pool: liquidation events, iAsset burnt, ADA received, realized premium, dilution.

use crate::chain::fetch::{KoiosAccountTx, KoiosAsset, KoiosTxUtxos};
use crate::indigo::events::{Event, EventKind};
use crate::indigo::protocol_config::IndigoV2Config;
use time::OffsetDateTime;

/// Reconstruct Stability Pool events from account txs and per-tx UTxO data.
/// When `config` has iasset_policy_ids or stability_pool_datum_hashes set, only UTxOs matching those are treated as SP.
pub fn reconstruct_stability_pool_events(
    account_txs: &[KoiosAccountTx],
    get_tx_utxos: impl Fn(&str) -> Option<KoiosTxUtxos>,
    now: OffsetDateTime,
    config: &IndigoV2Config,
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
            if !is_sp_utxo(out, config) {
                continue;
            }
            let out_ada = parse_lovelace(&out.value);
            if out_ada == 0 {
                continue;
            }
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

        for inp in inputs {
            if !is_sp_utxo(inp, config) {
                continue;
            }
            let in_ada = parse_lovelace(&inp.value);
            if in_ada == 0 {
                continue;
            }
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
    events.sort_by_key(|e| (e.slot.unwrap_or(0), e.tx_hash.clone()));
    events
}

/// True if this UTxO should be treated as Stability Pool (datum and iAsset policy match config when set).
fn is_sp_utxo(out: &crate::chain::fetch::KoiosUtxo, config: &IndigoV2Config) -> bool {
    if !config.is_stability_pool_datum(out.datum_hash.as_deref()) {
        return false;
    }
    let Some(assets) = out.asset_list.as_ref() else {
        return false;
    };
    let a = match assets.first() {
        Some(a) => a,
        None => return false,
    };
    config.is_known_iasset_policy(&a.policy_id)
}

fn parse_lovelace(s: &str) -> u64 {
    s.trim().parse::<u64>().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indigo::protocol_config::IndigoV2Config;

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
        let config = IndigoV2Config::default();
        let out = reconstruct_stability_pool_events(&txs, get_none, ts(), &config);
        assert!(out.is_empty());
    }
}
