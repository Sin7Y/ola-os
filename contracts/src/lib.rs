use std::{path::Path, fs};

use ola_types::{H256, U256};
use ola_utils::{bytecode::hash_bytecode, convert::bytes_to_be_words};

#[derive(Debug, Clone)]
pub struct SystemContractCode {
    pub code: Vec<U256>,
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
            code: bytes_to_be_words(entrypoint_bytecode),
            hash
        };

        let bytecode = read_sys_contract_bytecode("", "DefaultAccount");
        let hash = hash_bytecode(&bytecode);
        let default_aa = SystemContractCode {
            code: bytes_to_be_words(bytecode),
            hash,
        };

        BaseSystemContracts {
            entrypoint,
            default_aa,
        }
    }

    pub fn playground() -> Self {
        let entrypoint = read_proved_block_entrypoint_bytecode();
        BaseSystemContracts::load_with_entrypoint(entrypoint)
    }

    pub fn load_from_disk() -> Self {
        let entrypoint = read_proved_block_entrypoint_bytecode();
        BaseSystemContracts::load_with_entrypoint(entrypoint)
    }
}

pub fn read_zbin_bytecode(zbin_path: impl AsRef<Path>) -> Vec<u8> {
    let ola_home = std::env::var("OLA_HOME").unwrap_or_else(|_| {
        ".".into()
    });
    let bytecode_path = Path::new(&ola_home).join(zbin_path);
    fs::read(&bytecode_path).unwrap_or_else(|err| panic!("Failed reading .zbin bytecode at {:?}: {}", bytecode_path, err))
}

pub fn read_bootloader_code(bootloader_type: &str) -> Vec<u8> {
    read_zbin_bytecode(format!(""))
}

pub fn read_proved_block_entrypoint_bytecode() -> Vec<u8> {
    read_bootloader_code("bootloader_type")
}

pub fn read_sys_contract_bytecode(directory: &str, name: &str) -> Vec<u8> {
    // FIXME: repace zbin_path
    read_zbin_bytecode("zbin_path")
}
