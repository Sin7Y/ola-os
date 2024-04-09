use bigdecimal::BigDecimal;
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use num::{bigint::ToBigInt, rational::Ratio, BigUint};
use ola_basic_types::{Address, H256, U256};
use olavm_core::types::{GoldilocksField, account::Address as OlavmAddress, merkle_tree::{tree_key_to_h256, TreeValue, TreeKey}};
use olavm_plonky2::field::types::Field;

pub fn u256_to_big_decimal(value: U256) -> BigDecimal {
    let ratio = Ratio::new_raw(u256_to_biguint(value), BigUint::from(1u8));
    ratio_to_big_decimal(&ratio, 80)
}

pub fn ratio_to_big_decimal(num: &Ratio<BigUint>, precision: usize) -> BigDecimal {
    let bigint = round_precision_raw_no_div(num, precision)
        .to_bigint()
        .unwrap();
    BigDecimal::new(bigint, precision as i64)
}

fn round_precision_raw_no_div(num: &Ratio<BigUint>, precision: usize) -> BigUint {
    let ten_pow = BigUint::from(10u32).pow(precision as u32);
    (num * ten_pow).round().to_integer()
}

fn ensure_chunkable(bytes: &[u8]) {
    assert!(bytes.len() % 32 == 0, "Bytes must be divisible by 32")
}

pub fn bytes_to_chunks(bytes: &[u8]) -> Vec<[u8; 32]> {
    ensure_chunkable(bytes);
    bytes
        .chunks(32)
        .map(|byte_chunk| {
            let mut chunk = [0_u8; 32];
            chunk.copy_from_slice(byte_chunk);
            chunk
        })
        .collect()
}

pub fn bytes_to_be_words(bytes: Vec<u8>) -> Vec<U256> {
    ensure_chunkable(&bytes);
    bytes.chunks(32).map(U256::from_big_endian).collect()
}

pub fn address_to_h256(address: &Address) -> H256 {
    let mut buffer = [0u8; 32];
    buffer.copy_from_slice(address.as_bytes());
    H256(buffer)
}

pub fn olavm_address_to_address(addr: &OlavmAddress) -> Address {
    tree_key_to_h256(addr)
}

pub fn olavm_address_to_u256(addr: &OlavmAddress) -> U256 {
    h256_to_u256(tree_key_to_h256(addr))
}

pub fn u64s_to_u256(u64s: &[u64; 4]) -> U256 {
    h256_to_u256(u64_array_to_h256(u64s))
}

pub fn h160_bytes_to_h256(data: &[u8; 20]) -> H256 {
    let mut buffer = [0u8; 32];
    buffer[12..].copy_from_slice(data);
    H256(buffer)
}

pub fn h256_to_u256(num: H256) -> U256 {
    U256::from_big_endian(num.as_bytes())
}

pub fn u256_to_biguint(value: U256) -> BigUint {
    let mut bytes = [0u8; 32];
    value.to_little_endian(&mut bytes);
    BigUint::from_bytes_le(&bytes)
}

pub fn biguint_to_u256(value: BigUint) -> U256 {
    let bytes = value.to_bytes_le();
    U256::from_little_endian(&bytes)
}

pub fn bigdecimal_to_u256(value: BigDecimal) -> U256 {
    let bigint = value.with_scale(0).into_bigint_and_exponent().0;
    biguint_to_u256(bigint.to_biguint().unwrap())
}

pub fn u256_to_h256(num: U256) -> H256 {
    let mut bytes = [0u8; 32];
    num.to_big_endian(&mut bytes);
    H256::from_slice(&bytes)
}

pub fn h256_to_account_address(value: &H256) -> Address {
    Address::from_slice(&value.as_bytes())
}

pub fn h256_to_u32(value: H256) -> u32 {
    let be_u32_bytes: [u8; 4] = value[28..].try_into().unwrap();
    u32::from_be_bytes(be_u32_bytes)
}

pub fn h256_to_u64(value: H256) -> u64 {
    let be_u64_bytes: [u8; 8] = value[24..].try_into().unwrap();
    u64::from_be_bytes(be_u64_bytes)
}

pub fn be_words_to_bytes(words: &[U256]) -> Vec<u8> {
    words
        .iter()
        .flat_map(|w| {
            let mut bytes = [0u8; 32];
            w.to_big_endian(&mut bytes);
            bytes
        })
        .collect()
}

pub fn u64s_to_bytes(arr: &[u64]) -> Vec<u8> {
    arr.iter().flat_map(|w| w.to_be_bytes()).collect()
}

pub fn bytes_to_u64s(bytes: Vec<u8>) -> Vec<u64> {
    assert!(bytes.len() % 8 == 0, "Bytes must be divisible by 8");
    bytes
        .chunks(8)
        .map(|chunk| {
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(chunk);
            u64::from_be_bytes(bytes)
        })
        .collect()
}

pub fn h256_to_u64_array(h: &H256) -> [u64; 4] {
    let bytes = h.0;
    [
        u64::from_be_bytes(bytes[0..8].try_into().unwrap()),
        u64::from_be_bytes(bytes[8..16].try_into().unwrap()),
        u64::from_be_bytes(bytes[16..24].try_into().unwrap()),
        u64::from_be_bytes(bytes[24..32].try_into().unwrap()),
    ]
}

pub fn u64_array_to_h256(arr: &[u64; 4]) -> H256 {
    let mut bytes = [0u8; 32];
    for i in 0..arr.len() {
        bytes[i * 8..i * 8 + 8].clone_from_slice(&arr[i].to_be_bytes());
    }
    H256(bytes)
}

pub fn h256_to_string(h: &H256) -> String {
    let bytes = h.to_fixed_bytes();
    let s = hex::encode(bytes);
    s
}

pub fn program_bytecode_to_bytes(bytecode: &str) -> Option<Vec<u8>> {
    let felt_str_vec: Vec<_> = bytecode.split("\n").collect();
    let mut bytes = vec![];
    for felt_str in felt_str_vec {
        let mut hex_str = felt_str.trim_start_matches("0x").to_string();
        if hex_str.len() % 2 == 1 {
            hex_str = format!("0{}", hex_str);
        }
        let mut value = [0; 8];
        if let Ok(felt) = hex::decode(hex_str) {
            for (idx, &el) in felt.iter().rev().enumerate() {
                value[7 - idx] = el;
            }
            bytes.extend(value);
        } else {
            return None;
        }
    }
    Some(bytes)
}

pub fn serialize_block_number(block_number: u32) -> Vec<u8> {
    let mut bytes = vec![0; 4];
    BigEndian::write_u32(&mut bytes, block_number);
    bytes
}

pub fn deserialize_block_number(mut bytes: &[u8]) -> u32 {
    bytes
        .read_u32::<BigEndian>()
        .expect("failed to deserialize block number")
}

pub fn serialize_tree_leaf(leaf: TreeValue) -> Vec<u8> {
    let mut bytes = vec![0; 32];
    for (index, item) in leaf.iter().enumerate() {
        let field_array = item.0.to_be_bytes();
        bytes[index * 8..(index * 8 + 8)].copy_from_slice(&field_array);
    }
    bytes
}

pub fn serialize_leaf_index_to_key(leaf_index: u64) -> TreeKey {
    let key = [
        GoldilocksField::from_canonical_u64(leaf_index),
        GoldilocksField::ZERO,
        GoldilocksField::ZERO,
        GoldilocksField::ZERO,
    ];
    key
}

pub fn serialize_leaf_index(leaf_index: u64) -> Vec<u8> {
    let mut bytes = vec![0; 8];
    BigEndian::write_u64(&mut bytes, leaf_index);
    bytes
}

pub fn deserialize_leaf_index(mut bytes: &[u8]) -> u64 {
    bytes
        .read_u64::<BigEndian>()
        .expect("failed to deserialize leaf index")
}

#[cfg(test)]
mod tests {
    use ola_basic_types::H256;

    use crate::{h256_to_string, program_bytecode_to_bytes, u64s_to_bytes};

    #[test]
    fn test_program_bytecode_to_bytes() {
        let bytecode = "0x6000020080000000\n0xc\n0x6000020000200000";
        let expect = vec![
            0x60, 0x00, 0x02, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x0c, 0x60, 0x00, 0x02, 0x00, 0x00, 0x20, 0x00, 0x00,
        ];
        let real = program_bytecode_to_bytes(bytecode).unwrap();
        assert_eq!(expect, real);
    }

    #[test]
    fn test_u64s_to_u8s() {
        let u64s: Vec<u64> = vec![0, 1, 2];
        let result = u64s_to_bytes(&u64s);
        let expect: [u8; 24] = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2,
        ];
        assert_eq!(result.as_slice(), expect);
    }

    #[test]
    fn test_h256_to_string() {
        let hex_str = "1bcb518fd7c0176670f800a107ea75bb6ff31e83edc29700cbfcff40b06a0292";
        let bytes = hex::decode(hex_str).expect("failed to decode hex string");
        let h = H256::from_slice(&bytes);
        let s = h256_to_string(&h);
        assert_eq!(
            s.as_str(),
            "1bcb518fd7c0176670f800a107ea75bb6ff31e83edc29700cbfcff40b06a0292"
        );
    }
}
