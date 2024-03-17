use std::env;

use circuits::OlaBaseLayerCircuit;
use ola_types::{proofs::OlaBaseLayerProof, L1BatchNumber};
use serde::{Deserialize, Serialize};

pub mod circuits;

pub fn get_current_pod_name() -> String {
    env::var("OLAOS_POD_NAME").unwrap_or("UNKNOWN_POD".to_owned())
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProverJob {
    pub block_number: L1BatchNumber,
    pub job_id: u32,
    pub circuit_wrapper: CircuitWrapper,
    // pub setup_data_key: ProverServiceDataKey,
}

impl ProverJob {
    pub fn new(
        block_number: L1BatchNumber,
        job_id: u32,
        circuit_wrapper: CircuitWrapper,
        // setup_data_key: ProverServiceDataKey,
    ) -> Self {
        Self {
            block_number,
            job_id,
            circuit_wrapper,
            // setup_data_key,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum FriProofWrapper {
    Base(OlaBaseLayerProof),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum CircuitWrapper {
    Base(OlaBaseLayerCircuit),
}
