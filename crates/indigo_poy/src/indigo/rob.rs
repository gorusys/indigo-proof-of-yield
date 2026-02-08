//! Redemption Order Book: order placement, fills, premium/reimbursement %, cooldown.

use crate::chain::fetch::{KoiosAccountTx, KoiosTxUtxos};
use crate::indigo::events::{Event, EventKind};
use time::OffsetDateTime;

/// Reconstruct ROB-related events from account txs and tx UTxO data.
pub fn reconstruct_rob_events(
    account_txs: &[KoiosAccountTx],
    get_tx_utxos: impl Fn(&str) -> Option<KoiosTxUtxos>,
    now: OffsetDateTime,
) -> Vec<Event> {
    let mut events = Vec::new();
    for tx in account_txs {
        let slot = tx.slot_no;
        let ts = tx
            .block_time
            .and_then(|t| OffsetDateTime::from_unix_timestamp(t).ok())
            .unwrap_or(now);
        let tx_hash = tx.tx_hash.clone();

        let utxos = match get_tx_utxos(&tx_hash) {
            Some(u) => u,
            None => continue,
        };

        let inputs = utxos.inputs.as_deref().unwrap_or(&[]);
        let outputs = utxos.outputs.as_deref().unwrap_or(&[]);
        let in_ada: u64 = inputs.iter().map(|u| parse_lovelace(&u.value)).sum();
        let out_ada: u64 = outputs.iter().map(|u| parse_lovelace(&u.value)).sum();

        if out_ada > in_ada && in_ada > 0 {
            let premium = ((out_ada - in_ada) as f64 / in_ada as f64) * 100.0;
            events.push(Event {
                kind: EventKind::RobOrderFill {
                    order_id: None,
                    filled_lovelace: out_ada,
                    premium_pct: Some(premium),
                    reimbursement_pct: Some(premium),
                    tx_hash: tx_hash.clone(),
                    slot,
                },
                timestamp: ts,
                slot,
                tx_hash: tx_hash.clone(),
                extra: None,
            });
        } else if in_ada > 0 && out_ada == 0 {
            events.push(Event {
                kind: EventKind::RobOrderPlace {
                    order_id: None,
                    amount_lovelace: in_ada,
                    tx_hash: tx_hash.clone(),
                    slot,
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

fn parse_lovelace(s: &str) -> u64 {
    s.trim().parse::<u64>().unwrap_or(0)
}
