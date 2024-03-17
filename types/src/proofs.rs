use std::str::FromStr;

use ola_basic_types::{L1BatchNumber, H256, U256};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};

const HASH_LEN: usize = H256::len_bytes();

/// Metadata emitted by a Merkle tree after processing single storage log.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageLogMetadata {
    #[serde_as(as = "Bytes")]
    pub root_hash: [u8; HASH_LEN],
    pub pre_root_hash: [u8; HASH_LEN],
    pub is_write: bool,
    pub first_write: bool,
    #[serde_as(as = "Vec<Bytes>")]
    pub merkle_paths: Vec<[u8; HASH_LEN]>,
    pub leaf_hashed_key: U256,
    pub leaf_enumeration_index: u64,
    pub value_written: [u8; HASH_LEN],
    pub value_read: [u8; HASH_LEN],
}

impl StorageLogMetadata {
    pub fn leaf_hashed_key_array(&self) -> [u8; 32] {
        let mut result = [0_u8; 32];
        self.leaf_hashed_key.to_little_endian(&mut result);
        result
    }

    pub fn into_merkle_paths_array<const PATH_LEN: usize>(self) -> Box<[[u8; HASH_LEN]; PATH_LEN]> {
        let actual_len = self.merkle_paths.len();
        self.merkle_paths.try_into().unwrap_or_else(|_| {
            panic!(
                "Unexpected length of Merkle paths in `StorageLogMetadata`: expected {}, got {}",
                PATH_LEN, actual_len
            );
        })
    }
}

/// Represents the sequential number of the proof aggregation round.
/// Mostly used to be stored in `aggregation_round` column  in `prover_jobs` table
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AggregationRound {
    BasicCircuits = 0,
}

impl From<u8> for AggregationRound {
    fn from(item: u8) -> Self {
        match item {
            0 => AggregationRound::BasicCircuits,
            _ => panic!("Invalid round"),
        }
    }
}

impl AggregationRound {
    pub fn next(&self) -> Option<AggregationRound> {
        match self {
            AggregationRound::BasicCircuits => None,
        }
    }
}

impl std::fmt::Display for AggregationRound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::BasicCircuits => "basic_circuits",
        })
    }
}

impl FromStr for AggregationRound {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "basic_circuits" => Ok(AggregationRound::BasicCircuits),
            other => Err(format!(
                "{} is not a valid round name for witness generation",
                other
            )),
        }
    }
}

impl TryFrom<i32> for AggregationRound {
    type Error = ();

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            x if x == AggregationRound::BasicCircuits as i32 => Ok(AggregationRound::BasicCircuits),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareBasicCircuitsJob {
    // Merkle paths and some auxiliary information for each read / write operation in a block.
    merkle_paths: Vec<StorageLogMetadata>,
    next_enumeration_index: u64,
}

impl PrepareBasicCircuitsJob {
    /// Creates a new job with the specified leaf index and no included paths.
    pub fn new(next_enumeration_index: u64) -> Self {
        Self {
            merkle_paths: vec![],
            next_enumeration_index,
        }
    }

    /// Returns the next leaf index at the beginning of the block.
    pub fn next_enumeration_index(&self) -> u64 {
        self.next_enumeration_index
    }

    /// Reserves additional capacity for Merkle paths.
    pub fn reserve(&mut self, additional_capacity: usize) {
        self.merkle_paths.reserve(additional_capacity);
    }

    /// Pushes an additional Merkle path.
    pub fn push_merkle_path(&mut self, mut path: StorageLogMetadata) {
        let Some(first_path) = self.merkle_paths.first() else {
            self.merkle_paths.push(path);
            return;
        };
        assert_eq!(first_path.merkle_paths.len(), path.merkle_paths.len());

        let mut hash_pairs = path.merkle_paths.iter().zip(&first_path.merkle_paths);
        let first_unique_idx =
            hash_pairs.position(|(hash, first_path_hash)| hash != first_path_hash);
        let first_unique_idx = first_unique_idx.unwrap_or(path.merkle_paths.len());
        path.merkle_paths = path.merkle_paths.split_off(first_unique_idx);
        self.merkle_paths.push(path);
    }

    /// Converts this job into an iterator over the contained Merkle paths.
    pub fn into_merkle_paths(self) -> impl ExactSizeIterator<Item = StorageLogMetadata> {
        let mut merkle_paths = self.merkle_paths;
        if let [first, rest @ ..] = merkle_paths.as_mut_slice() {
            for path in rest {
                assert!(
                    path.merkle_paths.len() <= first.merkle_paths.len(),
                    "Merkle paths in `PrepareBasicCircuitsJob` are malformed; the first path is not \
                     the longest one"
                );
                let spliced_len = first.merkle_paths.len() - path.merkle_paths.len();
                let spliced_hashes = &first.merkle_paths[0..spliced_len];
                path.merkle_paths
                    .splice(0..0, spliced_hashes.iter().cloned());
                debug_assert_eq!(path.merkle_paths.len(), first.merkle_paths.len());
            }
        }
        merkle_paths.into_iter()
    }
}

/// Enriched `PrepareBasicCircuitsJob`. All the other fields are taken from the `l1_batches` table.
#[derive(Debug, Clone)]
pub struct BasicCircuitWitnessGeneratorInput {
    pub block_number: L1BatchNumber,
    pub previous_block_hash: H256,
    pub previous_block_timestamp: u64,
    pub block_timestamp: u64,
    pub used_bytecodes_hashes: Vec<U256>,
    // pub initial_heap_content: Vec<(usize, U256)>,
    pub merkle_paths_input: PrepareBasicCircuitsJob,
}

#[derive(Debug, Clone)]
pub struct FriProverJobMetadata {
    pub id: u32,
    pub block_number: L1BatchNumber,
    pub circuit_id: u8,
    pub aggregation_round: AggregationRound,
    pub sequence_number: usize,
    pub depth: u16,
    pub is_node_final_proof: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OlaBaseLayerProof {}
