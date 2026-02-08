//! indigo_poy — local-first proof-of-yield for Indigo Protocol.
//!
//! Reconstructs Stability Pool, ROB, and INDY staking outcomes from on-chain data.
//! Read-only; no seeds; no transaction signing.

pub mod chain;
pub mod compute;
pub mod indigo;
pub mod report;
pub mod verify;

pub use chain::fetch::{KoiosAccountTx, KoiosTxUtxos, KoiosUtxo};
pub use chain::{Cache, FetchConfig, Fetcher};
pub use compute::{compute_metrics, ComputeInput, ComputedMetrics};
pub use indigo::{Event, EventKind, IndigoEvents};
pub use report::ReportData;
pub use verify::{reproducibility_hash, EvidenceBundle, VerificationResult};
