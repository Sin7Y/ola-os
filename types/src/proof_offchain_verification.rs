use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct OffChainVerificationResult {
    pub l1_batch_number: u64,
    pub is_passed: bool,
}
