use ola_basic_types::{AccountTreeId, Address, U256};
use ola_config::constants::contracts::{
    ACCOUNT_CODE_STORAGE_ADDRESS, CONTRACT_DEPLOYER_ADDRESS, ENTRYPOINT_ADDRESS,
    KNOWN_CODES_STORAGE_ADDRESS, NONCE_HOLDER_ADDRESS,
};
use ola_contracts::read_sys_contract_bytecode;
use once_cell::sync::Lazy;

use crate::block::DeployedContract;

pub const TX_NONCE_INCREMENT: U256 = U256([1, 0, 0, 0]); // 1
pub const DEPLOYMENT_NONCE_INCREMENT: U256 = U256([0, 0, 1, 0]); // 2^128

static SYSTEM_CONTRACTS: Lazy<Vec<DeployedContract>> = Lazy::new(|| {
    let mut deployed_system_contracts = [
        ("", "AccountCodeStorage", ACCOUNT_CODE_STORAGE_ADDRESS),
        ("", "NonceHolder", NONCE_HOLDER_ADDRESS),
        ("", "KnownCodesStorage", KNOWN_CODES_STORAGE_ADDRESS),
        ("", "ContractDeployer", CONTRACT_DEPLOYER_ADDRESS),
    ]
    .map(|(path, name, address)| {
        let (raw, bytecode) = read_sys_contract_bytecode(path, name);
        DeployedContract {
            account_id: AccountTreeId::new(address),
            raw: raw.clone(),
            bytecode: bytecode.clone(),
        }
    })
    .to_vec();

    let (empty_raw, empty_bytecode) = read_sys_contract_bytecode("", "EmptyContract");
    // For now, only zero address and the bootloader address have empty bytecode at the init
    // In the future, we might want to set all of the system contracts this way.
    let empty_system_contracts =
        [Address::zero(), ENTRYPOINT_ADDRESS].map(|address| DeployedContract {
            account_id: AccountTreeId::new(address),
            raw: empty_raw.clone(),
            bytecode: empty_bytecode.clone(),
        });

    deployed_system_contracts.extend(empty_system_contracts);
    deployed_system_contracts
});

pub fn get_system_smart_contracts() -> Vec<DeployedContract> {
    SYSTEM_CONTRACTS.clone()
}
