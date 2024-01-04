use ethereum_types::{H256, H512, U256};
use sha2::{Digest, Sha256};

use crate::errors::NumberConvertError;

pub const OLA_FIELD_ORDER: u64 = 18446744069414584321; // 2^64-2^32+1

pub fn h256_from_hex_be(value: &str) -> anyhow::Result<H256> {
    let value = value.trim_start_matches("0x");

    let hex_chars_len = value.len();
    let expected_hex_length = 64;

    let parsed_bytes: [u8; 32] = if hex_chars_len == expected_hex_length {
        let mut buffer = [0u8; 32];
        hex::decode_to_slice(value, &mut buffer)?;
        buffer
    } else if hex_chars_len < expected_hex_length {
        let mut padded_hex = str::repeat("0", expected_hex_length - hex_chars_len);
        padded_hex.push_str(value);

        let mut buffer = [0u8; 32];
        hex::decode_to_slice(&padded_hex, &mut buffer)?;
        buffer
    } else {
        anyhow::bail!("Key out of range.");
    };
    Ok(H256(parsed_bytes))
}

pub fn h256_to_u64_array(h: H256) -> Result<[u64; 4], NumberConvertError> {
    let bytes = h.0;
    Ok([
        u64::from_be_bytes(
            bytes[0..8]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
        u64::from_be_bytes(
            bytes[8..16]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
        u64::from_be_bytes(
            bytes[16..24]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
        u64::from_be_bytes(
            bytes[24..32]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
    ])
}

pub fn h512_to_u64_array(h: H512) -> Result<[u64; 8], NumberConvertError> {
    let bytes = h.0;
    Ok([
        u64::from_be_bytes(
            bytes[0..8]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
        u64::from_be_bytes(
            bytes[8..16]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
        u64::from_be_bytes(
            bytes[16..24]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
        u64::from_be_bytes(
            bytes[24..32]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
        u64::from_be_bytes(
            bytes[32..40]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
        u64::from_be_bytes(
            bytes[40..48]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
        u64::from_be_bytes(
            bytes[48..56]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
        u64::from_be_bytes(
            bytes[56..64]
                .try_into()
                .map_err(|_| NumberConvertError::H256ToU64ArrayFailed(h.to_string()))?,
        ),
    ])
}

pub fn u256_to_u64_array_be(num: U256) -> [u64; 4] {
    return [num.0[3], num.0[2], num.0[1], num.0[0]];
}

pub fn is_u64_under_felt_order(num: u64) -> bool {
    num < OLA_FIELD_ORDER
}

pub fn is_h256_a_valid_ola_hash(h: H256) -> bool {
    match h256_to_u64_array(h) {
        Ok(arr) => arr.iter().all(|&num| is_u64_under_felt_order(num)),
        Err(_) => false,
    }
}

pub fn is_u256_a_valid_ola_hash(n: U256) -> bool {
    u256_to_u64_array_be(n)
        .iter()
        .all(|&num| is_u64_under_felt_order(num))
}

pub fn concat_h256_u32_and_sha256(h: H256, n: u32) -> H256 {
    let n_bytes = n.to_be_bytes();
    let mut hasher = Sha256::new();
    hasher.update(&h.0);
    hasher.update(&n_bytes);
    let result = hasher.finalize();
    H256::from_slice(&result)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use ethereum_types::H256;

    #[test]
    fn test_h256_to_u64_array() {
        let h =
            H256::from_str("0xAAAAAAAAAAAAAAAABBBBBBBBBBBBBBBBCCCCCCCCCCCCCCCCDDDDDDDDDDDDDDDD")
                .unwrap();
        let arr = h256_to_u64_array(h).unwrap();
        assert_eq!(arr[0], 0xAAAAAAAAAAAAAAAA);
        assert_eq!(arr[1], 0xBBBBBBBBBBBBBBBB);
        assert_eq!(arr[2], 0xCCCCCCCCCCCCCCCC);
        assert_eq!(arr[3], 0xDDDDDDDDDDDDDDDD)
    }

    #[test]
    fn test_is_h256_a_valid_ola_hash() {
        let h =
            H256::from_str("0xAAAAAAAAAAAAAAAABBBBBBBBBBBBBBBBCCCCCCCCCCCCCCCCDDDDDDDDDDDDDDDD")
                .unwrap();
        assert!(is_h256_a_valid_ola_hash(h));
        let h =
            H256::from_str("0xFFFFFFFFFFFFFFFFBBBBBBBBBBBBBBBBCCCCCCCCCCCCCCCCDDDDDDDDDDDDDDDE")
                .unwrap();
        assert!(!is_h256_a_valid_ola_hash(h));
    }

    #[test]
    fn test_u256_to_u64_array() {
        let num =
            U256::from_str("0x11AAAAAAAAAAAAAABBBBBBBBBBBBBBBBCCCCCCCCCCCCFFCCDDDDDDDDDDDDDDDD")
                .unwrap();
        let arr = u256_to_u64_array_be(num);
        assert_eq!(arr[0], 0x11AAAAAAAAAAAAAA);
        assert_eq!(arr[1], 0xBBBBBBBBBBBBBBBB);
        assert_eq!(arr[2], 0xCCCCCCCCCCCCFFCC);
        assert_eq!(arr[3], 0xDDDDDDDDDDDDDDDD)
    }
}
