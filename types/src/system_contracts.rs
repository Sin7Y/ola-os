use ola_basic_types::{AccountTreeId, Address, U256};
use ola_config::constants::contracts::BOOTLOADER_ADDRESS;
use ola_contracts::read_sys_contract_bytecode;
use once_cell::sync::Lazy;

use crate::block::DeployedContract;

pub const TX_NONCE_INCREMENT: U256 = U256([1, 0, 0, 0]); // 1
pub const DEPLOYMENT_NONCE_INCREMENT: U256 = U256([0, 0, 1, 0]); // 2^128

static SYSTEM_CONTRACTS: Lazy<Vec<DeployedContract>> = Lazy::new(|| {
    let mut deployed_system_contracts = [
        (
            "",
            "AccountCodeStorage",
            ACCOUNT_CODE_STORAGE_ADDRESS,
            ContractLanguage::Sol,
        ),
        (
            "",
            "NonceHolder",
            NONCE_HOLDER_ADDRESS,
            ContractLanguage::Sol,
        ),
        (
            "",
            "KnownCodesStorage",
            KNOWN_CODES_STORAGE_ADDRESS,
            ContractLanguage::Sol,
        ),
        (
            // FIXME: delete
            "",
            "ImmutableSimulator",
            IMMUTABLE_SIMULATOR_STORAGE_ADDRESS,
            ContractLanguage::Sol,
        ),
        (
            "",
            "ContractDeployer",
            CONTRACT_DEPLOYER_ADDRESS,
            ContractLanguage::Sol,
        ),
        (
            // FIXME: delete
            "",
            "L1Messenger",
            L1_MESSENGER_ADDRESS,
            ContractLanguage::Sol,
        ),
        (
            // FIXME: delete
            "",
            "MsgValueSimulator",
            MSG_VALUE_SIMULATOR_ADDRESS,
            ContractLanguage::Sol,
        ),
        (
            // FIXME: delete
            "",
            "L2EthToken",
            L2_ETH_TOKEN_ADDRESS,
            ContractLanguage::Sol,
        ),
        (
            // FIXME: delete
            "precompiles/",
            "Keccak256",
            KECCAK256_PRECOMPILE_ADDRESS,
            ContractLanguage::Yul,
        ),
        (
            // FIXME: delete
            "precompiles/",
            "SHA256",
            SHA256_PRECOMPILE_ADDRESS,
            ContractLanguage::Yul,
        ),
        (
            // FIXME: delete
            "precompiles/",
            "Ecrecover",
            ECRECOVER_PRECOMPILE_ADDRESS,
            ContractLanguage::Yul,
        ),
        (
            // FIXME: delete
            "",
            "SystemContext",
            SYSTEM_CONTEXT_ADDRESS,
            ContractLanguage::Sol,
        ),
        (
            // FIXME: delete
            "",
            "EventWriter",
            EVENT_WRITER_ADDRESS,
            ContractLanguage::Yul,
        ),
        (
            // FIXME: delete
            "",
            "BootloaderUtilities",
            BOOTLOADER_UTILITIES_ADDRESS,
            ContractLanguage::Sol,
        ),
        (
            // FIXME: delete
            "",
            "BytecodeCompressor",
            BYTECODE_COMPRESSOR_ADDRESS,
            ContractLanguage::Sol,
        ),
        (
            // FIXME: delete
            "",
            "ComplexUpgrader",
            COMPLEX_UPGRADER_ADDRESS,
            ContractLanguage::Sol,
        ),
    ]
    .map(|(path, name, address, contract_lang)| DeployedContract {
        account_id: AccountTreeId::new(address),
        bytecode: read_sys_contract_bytecode(path, name, contract_lang),
    })
    .to_vec();

    let empty_bytecode = read_sys_contract_bytecode("", "EmptyContract", ContractLanguage::Sol);
    // For now, only zero address and the bootloader address have empty bytecode at the init
    // In the future, we might want to set all of the system contracts this way.
    let empty_system_contracts =
        [Address::zero(), BOOTLOADER_ADDRESS].map(|address| DeployedContract {
            account_id: AccountTreeId::new(address),
            bytecode: empty_bytecode.clone(),
        });

    deployed_system_contracts.extend(empty_system_contracts);
    deployed_system_contracts
});

pub fn get_system_smart_contracts() -> Vec<DeployedContract> {
    SYSTEM_CONTRACTS.clone()
}
