use bigdecimal::BigDecimal;
use num::{bigint::ToBigInt, rational::Ratio, BigUint};
use ola_basic_types::{Address, H256, U256};

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
