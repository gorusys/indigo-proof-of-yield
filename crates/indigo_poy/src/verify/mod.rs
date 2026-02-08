//! Reproducibility hashing, manifest, and verification.

mod bundle;

pub use bundle::normalize_for_hash;
pub use bundle::{reproducibility_hash, EvidenceBundle, VerificationResult};
