use std::vec::Vec;

use ola_basic_types::L1BatchNumber;
use serde::{Deserialize, Serialize};

use crate::protocol_version::FriProtocolVersionId;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProofGenerationDataRequest {}

#[derive(Debug, Serialize, Deserialize)]
pub enum ProofGenerationDataResponse {
    Success(Option<ProofGenerationData>),
    Error(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitProofRequest {}

#[derive(Debug, Serialize, Deserialize)]
pub enum SubmitProofResponse {
    Success(Option<ProofGenerationData>),
    Error(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProofGenerationData {
    pub l1_batch_number: L1BatchNumber,
    // TODO:
    pub data: Vec<u8>,
    pub fri_protocol_version_id: FriProtocolVersionId,
}
