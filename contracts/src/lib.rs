use ola_basic_types::H256;
use ola_utils::{bytecode::hash_bytecode, program_bytecode_to_bytes};
use olavm_core::program::binary_program::BinaryProgram;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
};

#[derive(Debug, Clone)]
pub struct SystemContractCode {
    pub code: Vec<u8>,
    pub hash: H256,
}

#[derive(Debug, Clone)]
pub struct BaseSystemContracts {
    pub entrypoint: SystemContractCode,
    pub default_aa: SystemContractCode,
}

impl PartialEq for BaseSystemContracts {
    fn eq(&self, other: &Self) -> bool {
        self.entrypoint.hash == other.entrypoint.hash
            && self.default_aa.hash == other.default_aa.hash
    }
}

impl BaseSystemContracts {
    fn load_with_entrypoint(entrypoint_bytecode: Vec<u8>) -> Self {
        let hash = hash_bytecode(&entrypoint_bytecode);
        let entrypoint = SystemContractCode {
            code: entrypoint_bytecode,
            hash,
        };

        let (raw, _) = read_sys_contract_bytecode("", "DefaultAccount");
        let hash = hash_bytecode(&raw);
        let default_aa = SystemContractCode { code: raw, hash };

        BaseSystemContracts {
            entrypoint,
            default_aa,
        }
    }

    // used for eth_calls, the same as execute txs in Ola
    pub fn playground() -> Self {
        let (entrypoint, _) = read_proved_block_entrypoint_bytecode();
        BaseSystemContracts::load_with_entrypoint(entrypoint)
    }

    // used for execute txs in Ola
    pub fn load_from_disk() -> Self {
        let (entrypoint, _) = read_proved_block_entrypoint_bytecode();
        BaseSystemContracts::load_with_entrypoint(entrypoint)
    }

    pub fn hashes(&self) -> BaseSystemContractsHashes {
        BaseSystemContractsHashes {
            entrypoint: self.entrypoint.hash,
            default_aa: self.default_aa.hash,
        }
    }
}

pub fn read_json_program(json_path: impl AsRef<Path>) -> (Vec<u8>, Vec<u8>) {
    // dbg!(json_path.as_ref().to_str());
    let ola_home = std::env::var("OLAOS_HOME").unwrap_or_else(|_| ".".into());
    let bytecode_path = Path::new(&ola_home).join(json_path);
    let file = File::open(bytecode_path).unwrap();
    let reader = BufReader::new(file);
    let program: BinaryProgram = serde_json::from_reader(reader).unwrap();
    let json_bytes = bincode::serialize(&program).expect("failed to read system contracts");
    let bytecode_bytes =
        program_bytecode_to_bytes(&program.bytecode).expect("failed to load contract: {json_path}");
    (json_bytes, bytecode_bytes)
}

pub fn read_entrypoint_code(entrypoint_type: &str) -> (Vec<u8>, Vec<u8>) {
    read_json_program(format!(
        "etc/system-contracts/contracts/{}.json",
        entrypoint_type
    ))
}

pub fn read_proved_block_entrypoint_bytecode() -> (Vec<u8>, Vec<u8>) {
    read_entrypoint_code("Entrypoint")
}

pub fn read_sys_contract_bytecode(directory: &str, name: &str) -> (Vec<u8>, Vec<u8>) {
    read_json_program(format!(
        "etc/system-contracts/contracts/{0}{1}.json",
        directory, name
    ))
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct BaseSystemContractsHashes {
    pub entrypoint: H256,
    pub default_aa: H256,
}
