use ola_basic_types::H256;
use serde::{Deserialize, Serialize};

use crate::writes::{InitialStorageWrite, RepeatedStorageWrite};

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

// pub trait SerializeCommitment {
//     /// Size of the structure in bytes.
//     const SERIALIZED_SIZE: usize;
//     /// The number of objects of this type that can be included in a single L1 batch.
//     const LIMIT_PER_L1_BATCH: usize;
//     /// Serializes this struct into the provided buffer, which is guaranteed to have byte length
//     /// [`Self::SERIALIZED_SIZE`].
//     fn serialize_commitment(&self, buffer: &mut [u8]);
// }

// impl SerializeCommitment for InitialStorageWrite {
//     const SERIALIZED_SIZE: usize = 64;
//     const LIMIT_PER_L1_BATCH: usize =
//         GEOMETRY_CONFIG.limit_for_initial_writes_pubdata_hasher as usize;

//     fn serialize_commitment(&self, buffer: &mut [u8]) {
//         self.key.to_little_endian(&mut buffer[0..32]);
//         buffer[32..].copy_from_slice(self.value.as_bytes());
//     }
// }

// impl SerializeCommitment for RepeatedStorageWrite {
//     const SERIALIZED_SIZE: usize = 40;
//     const LIMIT_PER_L1_BATCH: usize =
//         GEOMETRY_CONFIG.limit_for_repeated_writes_pubdata_hasher as usize;

//     fn serialize_commitment(&self, buffer: &mut [u8]) {
//         buffer[..8].copy_from_slice(&self.index.to_be_bytes());
//         buffer[8..].copy_from_slice(self.value.as_bytes());
//     }
// }

// pub(crate) fn serialize_commitments<I: SerializeCommitment>(values: &[I]) -> Vec<u8> {
//     let final_len = values.len() * I::SERIALIZED_SIZE + 4;
//     let mut input = vec![0_u8; final_len];
//     input[0..4].copy_from_slice(&(values.len() as u32).to_be_bytes());

//     let chunks = input[4..].chunks_mut(I::SERIALIZED_SIZE);
//     for (value, chunk) in values.iter().zip(chunks) {
//         value.serialize_commitment(chunk);
//     }
//     input
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// struct RootState {
//     pub last_leaf_index: u64,
//     pub root_hash: H256,
// }

// #[derive(Debug, Clone, Serialize, Deserialize)]
// struct L1BatchPassThroughData {
//     shared_states: Vec<RootState>,
// }

// #[derive(Debug, Clone)]
// struct L1BatchAuxiliaryOutput {
//     #[allow(dead_code)]
//     initial_writes: Vec<InitialStorageWrite>,
//     #[allow(dead_code)]
//     repeated_writes: Vec<RepeatedStorageWrite>,
//     l2_l1_logs_compressed: Vec<u8>,
//     l2_l1_logs_linear_hash: H256,
//     l2_l1_logs_merkle_root: H256,
//     initial_writes_compressed: Vec<u8>,
//     initial_writes_hash: H256,
//     repeated_writes_compressed: Vec<u8>,
//     repeated_writes_hash: H256,
// }

// impl L1BatchAuxiliaryOutput {
//     fn new(
//         initial_writes: Vec<InitialStorageWrite>,
//         repeated_writes: Vec<RepeatedStorageWrite>,
//     ) -> Self {
//         let initial_writes_compressed = serialize_commitments(&initial_writes);
//         let repeated_writes_compressed = serialize_commitments(&repeated_writes);

//         let initial_writes_hash = H256::from(keccak256(&initial_writes_compressed));
//         let repeated_writes_hash = H256::from(keccak256(&repeated_writes_compressed));

//         let merkle_tree_leaves = l2_l1_logs_compressed[4..]
//             .chunks(L2ToL1Log::SERIALIZED_SIZE)
//             .map(|chunk| <[u8; L2ToL1Log::SERIALIZED_SIZE]>::try_from(chunk).unwrap());
//         // ^ Skip first 4 bytes of the serialized logs (i.e., the number of logs).
//         let l2_l1_logs_merkle_root =
//             MiniMerkleTree::new(merkle_tree_leaves, L2ToL1Log::LIMIT_PER_L1_BATCH).merkle_root();

//         Self {
//             l2_l1_logs_compressed,
//             initial_writes_compressed,
//             repeated_writes_compressed,
//             initial_writes,
//             repeated_writes,
//             l2_l1_logs_linear_hash,
//             l2_l1_logs_merkle_root,
//             initial_writes_hash,
//             repeated_writes_hash,
//         }
//     }
// }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct L1BatchMetaParameters {
    pub bootloader_code_hash: H256,
    pub default_aa_code_hash: H256,
}

// #[derive(Debug, Clone)]
// pub struct L1BatchCommitment {
//     pass_through_data: L1BatchPassThroughData,
//     auxiliary_output: L1BatchAuxiliaryOutput,
//     meta_parameters: L1BatchMetaParameters,
// }

// impl L1BatchCommitment {
//     pub fn new(
//         rollup_last_leaf_index: u64,
//         rollup_root_hash: H256,
//         initial_writes: Vec<InitialStorageWrite>,
//         repeated_writes: Vec<RepeatedStorageWrite>,
//         bootloader_code_hash: H256,
//         default_aa_code_hash: H256,
//     ) -> Self {
//         let meta_parameters = L1BatchMetaParameters {
//             bootloader_code_hash,
//             default_aa_code_hash,
//         };

//         Self {
//             pass_through_data: L1BatchPassThroughData {
//                 shared_states: vec![
//                     RootState {
//                         last_leaf_index: rollup_last_leaf_index,
//                         root_hash: rollup_root_hash,
//                     },
//                     // Despite the fact that zk_porter is not available we have to add params about it.
//                     RootState {
//                         last_leaf_index: 0,
//                         root_hash: H256::zero(),
//                     },
//                 ],
//             },
//             auxiliary_output: L1BatchAuxiliaryOutput::new(
//                 l2_to_l1_logs,
//                 initial_writes,
//                 repeated_writes,
//             ),
//             meta_parameters,
//         }
//     }
// }
