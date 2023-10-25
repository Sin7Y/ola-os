use ola_basic_types::{MiniblockNumber, H256};

pub fn miniblock_hash(miniblock_number: MiniblockNumber) -> H256 {
    // TODO:
    H256::default()
    // H256(keccak256(&miniblock_number.0.to_be_bytes()))
}
