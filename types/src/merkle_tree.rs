// use crate::vm::vm_state::Address;
use crate::{
    proofs::PrepareBasicCircuitsJob,
    storage::writes::{InitialStorageWrite, RepeatedStorageWrite},
};
use itertools::Itertools;
use olavm_plonky2::field::goldilocks_field::GoldilocksField;
use olavm_plonky2::field::types::Field;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use web3::types::{H160, H256, U256};

use olavm_core::types::merkle_tree::{
    constant::ROOT_TREE_DEPTH, TreeKey, TreeValue, ZkHash, GOLDILOCKS_FIELD_U8_LEN, TREE_VALUE_LEN,
};
pub type TreeKeyU256 = U256;

#[macro_export]
macro_rules! impl_from_wrapper {
    ($wrapper: ty, $inner: ty $(where for $(<$($gen: ident),+>)?: $($where: tt)+)?) => {
        impl $($(<$($gen),+>)*)? From<$inner> for $wrapper $(where $($where)+)? {
            fn from(inner: $inner) -> Self {
                Self(inner)
            }
        }

        impl $($(<$($gen),+>)*)? From<$wrapper> for $inner $(where $($where)+)? {
            fn from(wrapper: $wrapper) -> Self {
                wrapper.0
            }
        }
    };
    (deref $wrapper: ty, $inner: ty $(where for $(<$($gen: ident),+>)?: $($where: tt)+)?) => {
        $crate::impl_from_wrapper!($wrapper, $inner $(where for $(<$($gen),+>)*: $($where)+)?);

        impl $($(<$($gen),+>)*)? From<&$inner> for $wrapper $(where $($where)+)? {
            fn from(inner: &$inner) -> Self {
                Self(*inner)
            }
        }

        impl $($(<$($gen),+>)*)? From<&$wrapper> for $inner $(where $($where)+)? {
            fn from(wrapper: &$wrapper) -> Self {
                (*wrapper).0
            }
        }
    };
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize)]
pub struct LevelIndex(pub (u16, U256));

impl_from_wrapper!(LevelIndex, (u16, U256));

impl LevelIndex {
    pub fn bin_key(&self) -> Vec<u8> {
        bincode::serialize(&self).expect("Serialization failed")
    }
}

pub fn tree_key_default() -> TreeKey {
    [GoldilocksField::ZERO; TREE_VALUE_LEN]
}

pub fn tree_value_default() -> TreeValue {
    [GoldilocksField::ZERO; TREE_VALUE_LEN]
}

#[derive(Clone, Debug)]
pub enum NodeEntry {
    Branch {
        hash: TreeKey,
        left_hash: TreeKey,
        right_hash: TreeKey,
    },
    Leaf {
        hash: TreeKey,
    },
}

impl NodeEntry {
    pub fn hash(&self) -> &TreeKey {
        match self {
            NodeEntry::Branch { hash, .. } => hash,
            NodeEntry::Leaf { hash } => hash,
        }
    }

    pub fn into_hash(self) -> TreeKey {
        match self {
            NodeEntry::Branch { hash, .. } => hash,
            NodeEntry::Leaf { hash } => hash,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TreeOperation {
    Write {
        value: TreeValue,
        previous_value: TreeValue,
    },
    Read(TreeValue),
    Delete,
}

// TODO: These all need to be moved to Ola-os
const HASH_LEN: usize = 32;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageLogMetadata {
    pub root_hash: [u8; HASH_LEN],
    pub pre_root_hash: [u8; HASH_LEN],
    pub is_write: bool,
    pub first_write: bool,
    pub merkle_paths: Vec<[u8; HASH_LEN]>,
    pub leaf_hashed_key: U256,
    pub leaf_enumeration_index: u64,
    pub value_written: [u8; HASH_LEN],
    pub value_read: [u8; HASH_LEN],
}

#[derive(Debug, Clone, Default)]
pub struct TreeMetadata {
    pub root_hash: ZkHash,
    pub rollup_last_leaf_index: u64,
    pub initial_writes: Vec<InitialStorageWrite>,
    pub repeated_writes: Vec<RepeatedStorageWrite>,
    pub witness: Option<PrepareBasicCircuitsJob>,
}

#[derive(Debug, Clone, Default)]
pub struct LeafIndices {
    pub leaf_indices: HashMap<TreeKey, u64>,
    pub last_index: u64,
    pub previous_index: u64,
    pub initial_writes: Vec<InitialStorageWrite>,
    pub repeated_writes: Vec<RepeatedStorageWrite>,
}

pub fn tree_key_to_u256(value: &TreeKey) -> TreeKeyU256 {
    value
        .iter()
        .enumerate()
        .fold(TreeKeyU256::zero(), |acc, (_index, item)| {
            (acc << 64) + U256::from(item.0)
        })
}

pub fn u256_to_tree_key(value: &TreeKeyU256) -> TreeKey {
    value.0.iter().enumerate().fold(
        [GoldilocksField::ZERO; TREE_VALUE_LEN],
        |mut tree_key, (index, item)| {
            tree_key[TREE_VALUE_LEN - index - 1] = GoldilocksField::from_canonical_u64(*item);
            tree_key
        },
    )
}

pub fn h256_to_tree_key(value: &H256) -> TreeKey {
    u8_arr_to_tree_key(&value.0.to_vec())
}

pub fn h160_to_tree_key(value: H160) -> TreeKey {
    let value = H256::from(value);
    u8_arr_to_tree_key(&value.0.to_vec())
}

pub fn tree_key_to_h256(value: &TreeKey) -> H256 {
    let bytes: [u8; 32] = tree_key_to_u8_arr(value).try_into().expect(&format!(
        "Vec<u8> convert to [u8;32] failed with data {:?}",
        value
    ));
    H256(bytes)
}

pub fn h256_to_tree_value(value: &H256) -> TreeValue {
    h256_to_tree_key(value)
}

pub fn tree_value_to_h256(value: &TreeValue) -> H256 {
    tree_key_to_h256(value)
}

pub fn u8_arr_to_tree_key(value: &Vec<u8>) -> TreeKey {
    assert_eq!(
        value.len(),
        GOLDILOCKS_FIELD_U8_LEN * TREE_VALUE_LEN,
        "u8_array len is not equal TreeKey len"
    );
    value
        .iter()
        .chunks(GOLDILOCKS_FIELD_U8_LEN)
        .into_iter()
        .enumerate()
        .fold(
            [GoldilocksField::ZERO; TREE_VALUE_LEN],
            |mut tree_key, (index, chunk)| {
                tree_key[index] = GoldilocksField::from_canonical_u64(u64::from_be_bytes(
                    chunk
                        .map(|e| *e)
                        .collect::<Vec<_>>()
                        .try_into()
                        .expect("Convert u8 chunk to bytes failed"),
                ));
                tree_key
            },
        )
}

pub fn tree_key_to_u8_arr(value: &TreeKey) -> Vec<u8> {
    value.iter().fold(Vec::new(), |mut key_vec, item| {
        key_vec.extend(item.0.to_be_bytes().to_vec());
        key_vec
    })
}

// pub fn encode_addr(addr: &Address) -> String {
//     hex::encode(tree_key_to_u8_arr(addr))
// }

pub fn decode_addr(addr: String) -> TreeKey {
    u8_arr_to_tree_key(&hex::decode(addr).expect("Decode address from string failed"))
}

pub fn tree_key_to_leaf_index(value: &TreeKey) -> LevelIndex {
    let index = tree_key_to_u256(value);
    LevelIndex((ROOT_TREE_DEPTH as u16, index))
}
