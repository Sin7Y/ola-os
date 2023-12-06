use super::storage::writes::{InitialStorageWrite, RepeatedStorageWrite};
use ola_basic_types::H256;
use ola_utils::hash::hash_bytes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct L1BatchMetadata {
    pub root_hash: H256,
    pub rollup_last_leaf_index: u64,
    pub merkle_root_hash: H256,
    pub initial_writes_compressed: Vec<u8>,
    pub repeated_writes_compressed: Vec<u8>,
    pub commitment: H256,
    pub block_meta_params: L1BatchMetaParameters,
    pub aux_data_hash: H256,
    pub meta_parameters_hash: H256,
    pub pass_through_data_hash: H256,
}

pub trait SerializeCommitment {
    /// Size of the structure in bytes.
    const SERIALIZED_SIZE: usize;
    /// The number of objects of this type that can be included in a single L1 batch.
    const LIMIT_PER_L1_BATCH: usize;
    /// Serializes this struct into the provided buffer, which is guaranteed to have byte length
    /// [`Self::SERIALIZED_SIZE`].
    fn serialize_commitment(&self, buffer: &mut [u8]);
}

impl SerializeCommitment for InitialStorageWrite {
    const SERIALIZED_SIZE: usize = 64;
    // TODO:
    const LIMIT_PER_L1_BATCH: usize = 4765;

    fn serialize_commitment(&self, buffer: &mut [u8]) {
        self.key.to_little_endian(&mut buffer[0..32]);
        buffer[32..].copy_from_slice(self.value.as_bytes());
    }
}

impl SerializeCommitment for RepeatedStorageWrite {
    const SERIALIZED_SIZE: usize = 40;
    // TODO:
    const LIMIT_PER_L1_BATCH: usize = 7564;

    fn serialize_commitment(&self, buffer: &mut [u8]) {
        buffer[..8].copy_from_slice(&self.index.to_be_bytes());
        buffer[8..].copy_from_slice(self.value.as_bytes());
    }
}

pub(crate) fn serialize_commitments<I: SerializeCommitment>(values: &[I]) -> Vec<u8> {
    let final_len = values.len() * I::SERIALIZED_SIZE + 4;
    let mut input = vec![0_u8; final_len];
    input[0..4].copy_from_slice(&(values.len() as u32).to_be_bytes());

    let chunks = input[4..].chunks_mut(I::SERIALIZED_SIZE);
    for (value, chunk) in values.iter().zip(chunks) {
        value.serialize_commitment(chunk);
    }
    input
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RootState {
    pub last_leaf_index: u64,
    pub root_hash: H256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct L1BatchPassThroughData {
    shared_states: Vec<RootState>,
}

impl L1BatchPassThroughData {
    pub fn to_bytes(&self) -> Vec<u8> {
        // We assume that currently we have only one shared state: Rollup.
        const SERIALIZED_SIZE: usize = 8 + 32;
        let mut result = Vec::with_capacity(SERIALIZED_SIZE);
        assert_eq!(
            self.shared_states.len(),
            1,
            "Shared states' length is {} instead of 1",
            self.shared_states.len()
        );
        for state in self.shared_states.iter() {
            result.extend_from_slice(&state.last_leaf_index.to_be_bytes());
            result.extend_from_slice(state.root_hash.as_bytes());
        }
        assert_eq!(
            result.len(),
            SERIALIZED_SIZE,
            "Serialized size for BlockPassThroughData is bigger than expected"
        );
        result
    }

    pub fn hash(&self) -> H256 {
        hash_bytes(&self.to_bytes())
    }
}

#[derive(Debug, Clone)]
struct L1BatchAuxiliaryOutput {
    #[allow(dead_code)]
    initial_writes: Vec<InitialStorageWrite>,
    #[allow(dead_code)]
    repeated_writes: Vec<RepeatedStorageWrite>,
    initial_writes_compressed: Vec<u8>,
    initial_writes_hash: H256,
    repeated_writes_compressed: Vec<u8>,
    repeated_writes_hash: H256,
}

impl L1BatchAuxiliaryOutput {
    fn new(
        initial_writes: Vec<InitialStorageWrite>,
        repeated_writes: Vec<RepeatedStorageWrite>,
    ) -> Self {
        let initial_writes_compressed = serialize_commitments(&initial_writes);
        let repeated_writes_compressed = serialize_commitments(&repeated_writes);

        let initial_writes_hash = hash_bytes(&initial_writes_compressed);
        let repeated_writes_hash = hash_bytes(&repeated_writes_compressed);

        Self {
            initial_writes_compressed,
            repeated_writes_compressed,
            initial_writes,
            repeated_writes,
            initial_writes_hash,
            repeated_writes_hash,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        // 2 H256 values
        const SERIALIZED_SIZE: usize = 64;
        let mut result = Vec::with_capacity(SERIALIZED_SIZE);
        result.extend(self.initial_writes_hash.as_bytes());
        result.extend(self.repeated_writes_hash.as_bytes());
        result
    }

    pub fn hash(&self) -> H256 {
        hash_bytes(&self.to_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct L1BatchMetaParameters {
    pub bootloader_code_hash: H256,
    pub default_aa_code_hash: H256,
}

impl L1BatchMetaParameters {
    pub fn to_bytes(&self) -> Vec<u8> {
        const SERIALIZED_SIZE: usize = 4 + 32 + 32;
        let mut result = Vec::with_capacity(SERIALIZED_SIZE);
        result.extend(self.bootloader_code_hash.as_bytes());
        result.extend(self.default_aa_code_hash.as_bytes());
        result
    }

    pub fn hash(&self) -> H256 {
        hash_bytes(&self.to_bytes())
    }
}

#[derive(Debug, Clone)]
pub struct L1BatchCommitment {
    pass_through_data: L1BatchPassThroughData,
    auxiliary_output: L1BatchAuxiliaryOutput,
    meta_parameters: L1BatchMetaParameters,
}

#[derive(Debug, Clone)]
pub struct L1BatchCommitmentHash {
    pub pass_through_data: H256,
    pub aux_output: H256,
    pub meta_parameters: H256,
    pub commitment: H256,
}

impl L1BatchCommitment {
    pub fn new(
        rollup_last_leaf_index: u64,
        rollup_root_hash: H256,
        initial_writes: Vec<InitialStorageWrite>,
        repeated_writes: Vec<RepeatedStorageWrite>,
        bootloader_code_hash: H256,
        default_aa_code_hash: H256,
    ) -> Self {
        let meta_parameters = L1BatchMetaParameters {
            bootloader_code_hash,
            default_aa_code_hash,
        };

        Self {
            pass_through_data: L1BatchPassThroughData {
                shared_states: vec![RootState {
                    last_leaf_index: rollup_last_leaf_index,
                    root_hash: rollup_root_hash,
                }],
            },
            auxiliary_output: L1BatchAuxiliaryOutput::new(initial_writes, repeated_writes),
            meta_parameters,
        }
    }

    pub fn hash(&self) -> L1BatchCommitmentHash {
        let mut result = vec![];
        let pass_through_data_hash = self.pass_through_data.hash();
        result.extend_from_slice(pass_through_data_hash.as_bytes());
        let metadata_hash = self.meta_parameters.hash();
        result.extend_from_slice(metadata_hash.as_bytes());
        let auxiliary_output_hash = self.auxiliary_output.hash();
        result.extend_from_slice(auxiliary_output_hash.as_bytes());
        let commitment = hash_bytes(&result);
        L1BatchCommitmentHash {
            pass_through_data: pass_through_data_hash,
            aux_output: auxiliary_output_hash,
            meta_parameters: metadata_hash,
            commitment,
        }
    }

    pub fn meta_parameters(&self) -> L1BatchMetaParameters {
        self.meta_parameters.clone()
    }

    pub fn initial_writes_compressed(&self) -> &[u8] {
        &self.auxiliary_output.initial_writes_compressed
    }

    pub fn repeated_writes_compressed(&self) -> &[u8] {
        &self.auxiliary_output.repeated_writes_compressed
    }
}
