use ola_types::proofs::{PrepareBasicCircuitsJob, StorageLogMetadata};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PrecalculatedMerklePathsProvider {
    // We keep the root hash of the last processed leaf, as it is needed by the the witness generator.
    pub root_hash: [u8; 32],
    // TODO: use ola trace
    // The ordered list of expected leaves to be interacted with
    // pub pending_leaves: Vec<StorageLogMetadata>,
    // The index that would be assigned to the next new leaf
    // pub next_enumeration_index: u64,
    // For every Storage Write Log we expect two invocations: `get_leaf` and `insert_leaf`.
    // We set this flag to `true` after the initial `get_leaf` is invoked.
    pub is_get_leaf_invoked: bool,
}

impl PrecalculatedMerklePathsProvider {
    pub fn new(input: PrepareBasicCircuitsJob, root_hash: [u8; 32]) -> Self {
        // let next_enumeration_index = input.next_enumeration_index();
        // olaos_logs::info!("Initializing PrecalculatedMerklePathsProvider. Initial root_hash: {:?}, initial next_enumeration_index: {:?}", root_hash, next_enumeration_index);
        Self {
            root_hash,
            // pending_leaves: input.into_merkle_paths().collect(),
            // next_enumeration_index,
            is_get_leaf_invoked: false,
        }
    }
}
