//! Report data structure (HTML is generated in indigo_poy_report crate).

use crate::verify::EvidenceBundle;
use serde::{Deserialize, Serialize};

/// Data passed to the HTML report generator: bundle + reproducibility hash.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReportData {
    pub bundle: EvidenceBundle,
    pub reproducibility_hash_sha256: String,
}
