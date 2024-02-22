use std::str::FromStr;

use ola_basic_types::{L1BatchNumber, H256, U256};
use serde::{Deserialize, Serialize};

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
    // TODO:
    // merkle_paths: Vec<StorageLogMetadata>,
    next_enumeration_index: u64,
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
