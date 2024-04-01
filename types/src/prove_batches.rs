use serde::{Deserialize, Serialize};

use crate::{commitment::L1BatchWithMetadata, proofs::L1BatchProofForL1};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProveBatches {
    pub prev_l1_batch: L1BatchWithMetadata,
    pub l1_batches: Vec<L1BatchWithMetadata>,
    pub proofs: Vec<L1BatchProofForL1>,
    pub should_verify: bool,
}
