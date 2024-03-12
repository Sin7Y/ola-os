use ola_basic_types::{MiniblockNumber, H256};

use crate::hash::hash_bytes;

pub fn miniblock_hash(miniblock_number: MiniblockNumber) -> H256 {
    hash_bytes(&miniblock_number.0.to_be_bytes())
}

pub const fn ceil_div(a: u64, b: u64) -> u64 {
    if a == 0 {
        a
    } else {
        (a - 1) / b + 1
    }
}
