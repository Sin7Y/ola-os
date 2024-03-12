use ola_basic_types::{Address, H256, U256};
use serde::{Deserialize, Serialize};

/// Total byte size of all fields in StateDiffRecord struct
/// 20 + 32 + 32 + 8 + 32 + 32
const STATE_DIFF_RECORD_SIZE: usize = 156;

// 2 * 136 - the size that allows for two keccak rounds.
pub const PADDED_ENCODED_STORAGE_DIFF_LEN_BYTES: usize = 272;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InitialStorageWrite {
    pub index: u64,
    pub key: U256,
    pub value: H256,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default, Eq, PartialEq)]
pub struct RepeatedStorageWrite {
    pub index: u64,
    pub value: H256,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default, Eq, PartialEq)]
pub struct StateDiffRecord {
    /// address state diff occurred at
    pub address: Address,
    /// storage slot key updated
    pub key: U256,
    /// derived_key == Blake2s(bytes32(address), key)
    pub derived_key: [u8; 32],
    /// index in tree of state diff
    pub enumeration_index: u64,
    /// previous value
    pub initial_value: U256,
    /// updated value
    pub final_value: U256,
}

impl StateDiffRecord {
    // Serialize into byte representation.
    fn encode(&self) -> [u8; STATE_DIFF_RECORD_SIZE] {
        let mut encoding = [0u8; STATE_DIFF_RECORD_SIZE];
        let mut offset = 0;
        let mut end = 0;

        end += 20;
        encoding[offset..end].copy_from_slice(self.address.as_fixed_bytes());
        offset = end;

        end += 32;
        self.key.to_big_endian(&mut encoding[offset..end]);
        offset = end;

        end += 32;
        encoding[offset..end].copy_from_slice(&self.derived_key);
        offset = end;

        end += 8;
        encoding[offset..end].copy_from_slice(&self.enumeration_index.to_be_bytes());
        offset = end;

        end += 32;
        self.initial_value.to_big_endian(&mut encoding[offset..end]);
        offset = end;

        end += 32;
        self.final_value.to_big_endian(&mut encoding[offset..end]);
        offset = end;

        debug_assert_eq!(offset, encoding.len());

        encoding
    }

    pub fn encode_padded(&self) -> [u8; PADDED_ENCODED_STORAGE_DIFF_LEN_BYTES] {
        let mut extended_state_diff_encoding = [0u8; PADDED_ENCODED_STORAGE_DIFF_LEN_BYTES];
        let packed_encoding = self.encode();
        extended_state_diff_encoding[0..packed_encoding.len()].copy_from_slice(&packed_encoding);

        extended_state_diff_encoding
    }

    /// Decode bytes into StateDiffRecord
    pub fn try_from_slice(data: &[u8]) -> Option<Self> {
        if data.len() == 156 {
            Some(Self {
                address: Address::from_slice(&data[0..20]),
                key: U256::from(&data[20..52]),
                derived_key: data[52..84].try_into().unwrap(),
                enumeration_index: u64::from_be_bytes(data[84..92].try_into().unwrap()),
                initial_value: U256::from(&data[92..124]),
                final_value: U256::from(&data[124..156]),
            })
        } else {
            None
        }
    }
}
