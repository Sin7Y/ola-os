use ola_basic_types::H256;
use olavm_plonky2::hash::utils::poseidon_hash_bytes;

pub fn hash_bytes(inputs: &[u8]) -> H256 {
    let hash = poseidon_hash_bytes(inputs);
    H256::from(hash)
}
