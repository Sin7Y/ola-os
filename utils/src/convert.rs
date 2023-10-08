use ola_types::U256;

fn ensure_chunkable(bytes: &[u8]) {
    assert!(
        bytes.len() % 32 == 0,
        "Bytes must be divisible by 32"
    )
}

pub fn bytes_to_chunks(bytes: &[u8]) -> Vec<[u8; 32]> {
    ensure_chunkable(bytes);
    bytes.chunks(32).map(|byte_chunk| {
        let mut chunk = [0_u8; 32];
        chunk.copy_from_slice(byte_chunk);
        chunk
    }).collect()
}

pub fn bytes_to_be_words(bytes: Vec<u8>) -> Vec<U256> {
    ensure_chunkable(&bytes);
    bytes.chunks(32).map(U256::from_big_endian).collect()
}