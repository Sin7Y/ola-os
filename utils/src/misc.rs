use ola_basic_types::{blake3, MiniblockNumber, H256};

pub fn miniblock_hash(miniblock_number: MiniblockNumber) -> H256 {
    let hash = blake3::hash(&miniblock_number.0.to_be_bytes());
    H256::from(hash.as_bytes())
}
