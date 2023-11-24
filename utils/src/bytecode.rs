use ola_basic_types::{blake3, H256};

const MAX_BYTECODE_LENGTH_IN_WORDS: usize = (1 << 16) - 1;
const MAX_BYTECODE_LENGTH_BYTES: usize = MAX_BYTECODE_LENGTH_IN_WORDS * 32;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum InvalidBytecodeError {
    #[error("Bytecode too long: {0} bytes, while max {1} allowed")]
    BytecodeTooLong(usize, usize),
    #[error("Bytecode has even number of 32-byte words")]
    BytecodeLengthInWordsIsEven,
    #[error("Bytecode length is not divisible by 32")]
    BytecodeLengthIsNotDivisibleBy32,
}

pub fn hash_bytecode(code: &[u8]) -> H256 {
    // FIXME: check bytecode hash
    let hash = blake3::hash(code);
    H256::from(hash.as_bytes())
}

pub fn validate_bytecode(code: &[u8]) -> Result<(), InvalidBytecodeError> {
    // TODO: check
    let bytecode_len = code.len();

    if bytecode_len > MAX_BYTECODE_LENGTH_BYTES {
        return Err(InvalidBytecodeError::BytecodeTooLong(
            bytecode_len,
            MAX_BYTECODE_LENGTH_BYTES,
        ));
    }

    if bytecode_len % 32 != 0 {
        return Err(InvalidBytecodeError::BytecodeLengthIsNotDivisibleBy32);
    }

    let bytecode_len_words = bytecode_len / 32;

    if bytecode_len_words % 2 == 0 {
        return Err(InvalidBytecodeError::BytecodeLengthInWordsIsEven);
    }

    Ok(())
}

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct CompressedBytecodeInfo {
//     pub original: Vec<u64>,
//     pub compressed: Vec<u64>,
// }

// impl CompressedBytecodeInfo {
//     pub fn from_original(bytecode: Vec<u8>) -> Result<Self, FailedToCompressBytecodeError> {
//         let compressed = compress_bytecode(&bytecode)?;

//         let result = Self {
//             original: bytecode,
//             compressed,
//         };

//         Ok(result)
//     }
// }

// #[derive(Debug, thiserror::Error)]
// pub enum FailedToCompressBytecodeError {
//     #[error("Number of unique 8-bytes bytecode chunks exceed the limit of 2^16 - 1")]
//     DictionaryOverflow,
//     #[error("Bytecode is invalid: {0}")]
//     InvalidBytecode(#[from] InvalidBytecodeError),
// }

// pub fn compress_bytecode(code: &[u8]) -> Result<Vec<u8>, FailedToCompressBytecodeError> {
//     validate_bytecode(code)?;

//     // Statistic is a hash map of values (number of occurences, first occurence position),
//     // this is needed to ensure that the determinism during sorting of the statistic, i.e.
//     // each element will have unique first occurence position
//     let mut statistic: HashMap<u64, (usize, usize)> = HashMap::new();
//     let mut dictionary: HashMap<u64, u16> = HashMap::new();
//     let mut encoded_data: Vec<u8> = Vec::new();

//     // Split original bytecode into 8-byte chunks.
//     for (position, chunk_bytes) in code.chunks(8).enumerate() {
//         // It is safe to unwrap here, because each chunk is exactly 8 bytes, since
//         // valid bytecodes are divisible by 8.
//         let chunk = u64::from_be_bytes(chunk_bytes.try_into().unwrap());

//         // Count the number of occurrences of each chunk.
//         statistic.entry(chunk).or_insert((0, position)).0 += 1;
//     }

//     let mut statistic_sorted_by_value: Vec<_> = statistic.into_iter().collect::<Vec<_>>();
//     statistic_sorted_by_value.sort_by_key(|x| x.1);

//     // The dictionary size is limited by 2^16 - 1,
//     if statistic_sorted_by_value.len() > u16::MAX.into() {
//         return Err(FailedToCompressBytecodeError::DictionaryOverflow);
//     }

//     // Fill the dictionary with the pmost popular chunks.
//     // The most popular chunks will be encoded with the smallest indexes, so that
//     // the 255 most popular chunks will be encoded with one zero byte.
//     // And the encoded data will be filled with more zeros, so
//     // the calldata that will be sent to L1 will be cheaper.
//     for (chunk, _) in statistic_sorted_by_value.iter().rev() {
//         dictionary.insert(*chunk, dictionary.len() as u16);
//     }

//     for chunk_bytes in code.chunks(8) {
//         // It is safe to unwrap here, because each chunk is exactly 8 bytes, since
//         // valid bytecodes are divisible by 8.
//         let chunk = u64::from_be_bytes(chunk_bytes.try_into().unwrap());

//         // Add the index of the chunk to the encoded data.
//         encoded_data.extend(dictionary.get(&chunk).unwrap().to_be_bytes());
//     }

//     // Prepare the raw compressed bytecode in the following format:
//     // - 2 bytes: the length of the dictionary (N)
//     // - N bytes: packed dictionary bytes
//     // - remaining bytes: packed encoded data bytes

//     let mut compressed: Vec<u8> = Vec::new();
//     compressed.extend((dictionary.len() as u16).to_be_bytes());

//     dictionary
//         .into_iter()
//         .map(|(k, v)| (v, k))
//         .sorted()
//         .for_each(|(_, chunk)| {
//             compressed.extend(chunk.to_be_bytes());
//         });

//     compressed.extend(encoded_data);

//     Ok(compressed)
// }
