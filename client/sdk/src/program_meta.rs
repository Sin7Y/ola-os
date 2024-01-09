use std::{fs::File, io::Read, path::PathBuf};

use ethereum_types::{H256, U256};
use ola_core::{
    crypto::poseidon_trace::calculate_arbitrary_poseidon, program::binary_program::BinaryProgram,
    types::GoldilocksField,
};
use ola_utils::{hash::PoseidonBytes, u256_to_h256};

#[derive(Debug)]
pub struct ProgramMeta {
    pub bytes: Vec<u8>,
    pub instructions: Vec<u64>,
    pub program_hash: H256,
    pub bytecode_hash: H256,
}

impl ProgramMeta {
    pub fn new(
        bytes: Vec<u8>,
        instructions: Vec<u64>,
        program_hash: H256,
        bytecode_hash: H256,
    ) -> Self {
        Self {
            bytes,
            instructions,
            program_hash,
            bytecode_hash,
        }
    }

    pub fn from_file(path: PathBuf) -> anyhow::Result<Self> {
        let mut program_file = File::open(path.clone())?;
        let program: BinaryProgram = serde_json::from_reader(File::open(path)?)?;
        let mut program_bytes = Vec::new();
        let _ = program_file.read_to_end(&mut program_bytes)?;
        let program_hash = program_bytes.hash_bytes();
        let instructions_u64 = program.bytecode_u64_array()?;
        let instructions: Vec<GoldilocksField> = instructions_u64
            .iter()
            .map(|n| GoldilocksField(*n))
            .collect();
        let bytecode_hash_u256 = calculate_arbitrary_poseidon(&instructions).map(|fe| fe.0);
        let bytecode_hash = u256_to_h256(U256(bytecode_hash_u256));
        Ok(Self::new(
            program_bytes,
            instructions_u64,
            H256(program_hash),
            bytecode_hash,
        ))
    }
}
