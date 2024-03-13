use ola_types::L1BatchNumber;
use sqlx::types::chrono::NaiveDateTime;

pub struct StorageOffChainVerifyDetails {
    pub l1_batch_number: i64,
    pub status: String,
    pub verifier_picked_at: Option<NaiveDateTime>,
    pub verifier_submit_at: Option<NaiveDateTime>,
}
