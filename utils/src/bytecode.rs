use ola_types::H256;

use crate::convert::bytes_to_chunks;

pub fn hash_bytecode(code: &[u8]) -> H256 {
    let chunked_code = bytes_to_chunks(code);
    // FIXME: calculate hash
    let hash = [0u8; 32];
    H256(hash)
}
