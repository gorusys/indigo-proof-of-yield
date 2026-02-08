//! Computed metrics: PnL, APR, realized premium, dilution.

use crate::indigo::{EventKind, IndigoEvents};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComputeInput {
    pub events: IndigoEvents,
    /// Period start for APR (Unix timestamp).
    pub period_start_ts: Option<i64>,
    /// Period end for APR (Unix timestamp).
    pub period_end_ts: Option<i64>,
    /// Current total ADA in position (lovelace) if known.
    pub current_ada_position: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DilutionModel {
    pub total_iasset_at_risk: Option<String>,
    pub user_share_pct: Option<f64>,
    pub dilution_effect_lovelace: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComputedMetrics {
    pub stability_pool: StabilityPoolMetrics,
    pub rob: RobMetrics,
    pub indy_staking: IndyStakingMetrics,
    pub combined: CombinedMetrics,
    pub dilution: Option<DilutionModel>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StabilityPoolMetrics {
    pub total_deposits_lovelace: u64,
    pub total_withdrawals_lovelace: u64,
    pub total_liquidations_ada_received_lovelace: u64,
    pub total_realized_premium_lovelace: u64,
    pub net_ada_from_liquidations_lovelace: i64,
    pub liquidation_count: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RobMetrics {
    pub total_placed_lovelace: u64,
    pub total_filled_lovelace: u64,
    pub total_premium_received_lovelace: u64,
    pub avg_premium_pct: Option<f64>,
    pub fill_count: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IndyStakingMetrics {
    pub total_rewards_lovelace: u64,
    pub total_sp_premium_lovelace: u64,
    pub reward_tx_count: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CombinedMetrics {
    pub total_ada_in_lovelace: u64,
    pub total_ada_out_lovelace: u64,
    pub net_pnl_lovelace: i64,
    pub apr_pct: Option<f64>,
}

/// Compute all metrics from events and optional period/position.
pub fn compute_metrics(input: &ComputeInput) -> ComputedMetrics {
    let mut sp = StabilityPoolMetrics::default();
    let mut rob = RobMetrics::default();
    let mut indy = IndyStakingMetrics::default();
    let mut combined = CombinedMetrics::default();

    let mut total_in: u64 = 0;
    let mut total_out: u64 = 0;

    for ev in input.events.all_events() {
        match &ev.kind {
            EventKind::StabilityPoolDeposit {
                amount_lovelace, ..
            } => {
                sp.total_deposits_lovelace =
                    sp.total_deposits_lovelace.saturating_add(*amount_lovelace);
                total_in = total_in.saturating_add(*amount_lovelace);
            }
            EventKind::StabilityPoolWithdraw {
                amount_lovelace, ..
            } => {
                sp.total_withdrawals_lovelace = sp
                    .total_withdrawals_lovelace
                    .saturating_add(*amount_lovelace);
                total_out = total_out.saturating_add(*amount_lovelace);
            }
            EventKind::StabilityPoolLiquidation {
                ada_received_lovelace,
                realized_premium_lovelace,
                ..
            } => {
                sp.total_liquidations_ada_received_lovelace = sp
                    .total_liquidations_ada_received_lovelace
                    .saturating_add(*ada_received_lovelace);
                sp.total_realized_premium_lovelace = sp
                    .total_realized_premium_lovelace
                    .saturating_add(*realized_premium_lovelace);
                sp.liquidation_count = sp.liquidation_count.saturating_add(1);
                total_out = total_out.saturating_add(*ada_received_lovelace);
            }
            EventKind::RobOrderPlace {
                amount_lovelace, ..
            } => {
                rob.total_placed_lovelace =
                    rob.total_placed_lovelace.saturating_add(*amount_lovelace);
                total_in = total_in.saturating_add(*amount_lovelace);
            }
            EventKind::RobOrderFill {
                filled_lovelace,
                premium_pct,
                ..
            } => {
                let premium = (*filled_lovelace as f64) * premium_pct.unwrap_or(0.0) / 100.0;
                rob.total_filled_lovelace =
                    rob.total_filled_lovelace.saturating_add(*filled_lovelace);
                rob.total_premium_received_lovelace = rob
                    .total_premium_received_lovelace
                    .saturating_add(premium as u64);
                rob.fill_count = rob.fill_count.saturating_add(1);
                total_out = total_out.saturating_add(*filled_lovelace);
                if let Some(p) = premium_pct {
                    rob.avg_premium_pct = Some(rob.avg_premium_pct.map_or(*p, |a| (a + p) / 2.0));
                }
            }
            EventKind::IndyStakingReward {
                amount_lovelace, ..
            } => {
                indy.total_rewards_lovelace =
                    indy.total_rewards_lovelace.saturating_add(*amount_lovelace);
                indy.reward_tx_count = indy.reward_tx_count.saturating_add(1);
                total_out = total_out.saturating_add(*amount_lovelace);
            }
            EventKind::IndySpPremium {
                amount_lovelace, ..
            } => {
                indy.total_sp_premium_lovelace = indy
                    .total_sp_premium_lovelace
                    .saturating_add(*amount_lovelace);
                total_out = total_out.saturating_add(*amount_lovelace);
            }
            _ => {}
        }
    }

    sp.net_ada_from_liquidations_lovelace =
        sp.total_liquidations_ada_received_lovelace
            .saturating_sub(sp.total_deposits_lovelace) as i64;

    combined.total_ada_in_lovelace = total_in;
    combined.total_ada_out_lovelace = total_out;
    combined.net_pnl_lovelace = total_out as i64 - total_in as i64;

    if let (Some(start), Some(end)) = (input.period_start_ts, input.period_end_ts) {
        let period_secs = (end - start).max(1) as f64;
        let position = input
            .current_ada_position
            .unwrap_or(total_in.saturating_sub(total_out))
            .max(1) as f64;
        let pnl = combined.net_pnl_lovelace.max(0) as f64;
        combined.apr_pct = Some((pnl / position) * (365.25 * 24.0 * 3600.0 / period_secs) * 100.0);
    }

    ComputedMetrics {
        stability_pool: sp,
        rob,
        indy_staking: indy,
        combined,
        dilution: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indigo::{Event, EventKind};
    use time::OffsetDateTime;

    #[test]
    fn compute_empty() {
        let input = ComputeInput::default();
        let m = compute_metrics(&input);
        assert_eq!(m.combined.total_ada_in_lovelace, 0);
        assert_eq!(m.combined.net_pnl_lovelace, 0);
    }

    #[test]
    fn compute_apr() {
        let mut events = IndigoEvents::default();
        events.stability_pool.push(Event {
            kind: EventKind::StabilityPoolLiquidation {
                iasset_burnt: "x".into(),
                ada_received_lovelace: 1_100_000,
                realized_premium_lovelace: 100_000,
                dilution_effect: None,
                tx_hash: "abc".into(),
                slot: Some(100),
            },
            timestamp: OffsetDateTime::from_unix_timestamp(1000).unwrap(),
            slot: Some(100),
            tx_hash: "abc".into(),
            extra: None,
        });
        let input = ComputeInput {
            period_start_ts: Some(0),
            period_end_ts: Some(365 * 24 * 3600),
            current_ada_position: Some(1_000_000),
            events,
        };
        let m = compute_metrics(&input);
        assert!(m.combined.apr_pct.is_some());
        assert!(m.stability_pool.liquidation_count == 1);
    }

    #[test]
    fn ordering_invariance() {
        let mut a = IndigoEvents::default();
        a.stability_pool.push(Event {
            kind: EventKind::StabilityPoolDeposit {
                amount_lovelace: 100,
                iasset_amount: None,
                tx_hash: "a".into(),
            },
            timestamp: OffsetDateTime::from_unix_timestamp(1).unwrap(),
            slot: Some(1),
            tx_hash: "a".into(),
            extra: None,
        });
        a.stability_pool.push(Event {
            kind: EventKind::StabilityPoolWithdraw {
                amount_lovelace: 50,
                iasset_amount: None,
                tx_hash: "b".into(),
            },
            timestamp: OffsetDateTime::from_unix_timestamp(2).unwrap(),
            slot: Some(2),
            tx_hash: "b".into(),
            extra: None,
        });
        let mut b = IndigoEvents::default();
        b.stability_pool.push(a.stability_pool[1].clone());
        b.stability_pool.push(a.stability_pool[0].clone());
        let in1 = ComputeInput {
            events: a,
            period_start_ts: None,
            period_end_ts: None,
            current_ada_position: None,
        };
        let in2 = ComputeInput {
            events: b,
            period_start_ts: None,
            period_end_ts: None,
            current_ada_position: None,
        };
        let m1 = compute_metrics(&in1);
        let m2 = compute_metrics(&in2);
        assert_eq!(
            m1.combined.total_ada_in_lovelace,
            m2.combined.total_ada_in_lovelace
        );
        assert_eq!(
            m1.combined.total_ada_out_lovelace,
            m2.combined.total_ada_out_lovelace
        );
        assert_eq!(m1.combined.net_pnl_lovelace, m2.combined.net_pnl_lovelace);
    }
}
