use super::BlockDetailsBase;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use zksync_basic_types::L1BatchNumber;

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OffChainVerificationResult {
    pub l1_batch_number: u64,
    pub is_passed: bool,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OffChainVerificationDetails {
    pub l1_batch_number: L1BatchNumber,
    pub verifier_status: String,
    pub verifier_picked_at: Option<DateTime<Utc>>,
    pub verifier_submit_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L1BatchDetailsWithOffchainVerification {
    pub number: L1BatchNumber,
    #[serde(flatten)]
    pub base: BlockDetailsBase,
    #[serde(flatten)]
    pub offchain_verification: OffChainVerificationDetails,
}
