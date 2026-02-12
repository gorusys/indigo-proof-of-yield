//! PnL, APR, realized premium, dilution math.

mod metrics;

pub use metrics::DilutionModel;
pub use metrics::{
    compute_metrics, CombinedMetrics, ComputeInput, ComputedMetrics, IndyStakingMetrics,
    RobMetrics, StabilityPoolMetrics,
};
