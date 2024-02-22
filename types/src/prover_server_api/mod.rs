use std::vec::Vec;

use ola_basic_types::L1BatchNumber;
use serde::{Deserialize, Serialize};

use crate::{
    proofs::PrepareBasicCircuitsJob,
    protocol_version::{FriProtocolVersionId, L1VerifierConfig},
};

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
    Success,
    Error(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProofGenerationData {
    pub l1_batch_number: L1BatchNumber,
    pub data: PrepareBasicCircuitsJob,
    pub fri_protocol_version_id: FriProtocolVersionId,
    pub l1_verifier_config: L1VerifierConfig,
}
