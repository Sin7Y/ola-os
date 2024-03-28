use std::convert::TryInto;

use ola_contracts::BaseSystemContractsHashes;
use ola_types::protocol_version::{L1VerifierConfig, ProtocolUpgradeTx, VerifierParams};
use ola_types::{api, protocol_version, H256};
use sqlx::types::chrono::NaiveDateTime;

#[derive(sqlx::FromRow)]
pub struct StorageProtocolVersion {
    pub id: i32,
    pub timestamp: i64,
    pub recursion_scheduler_level_vk_hash: Vec<u8>,
    pub recursion_node_level_vk_hash: Vec<u8>,
    pub recursion_leaf_level_vk_hash: Vec<u8>,
    pub recursion_circuits_set_vks_hash: Vec<u8>,
    pub bootloader_code_hash: Vec<u8>,
    pub default_account_code_hash: Vec<u8>,
    pub verifier_address: Vec<u8>,
    pub created_at: NaiveDateTime,
    pub upgrade_tx_hash: Option<Vec<u8>>,
}

impl From<StorageProtocolVersion> for api::ProtocolVersion {
    fn from(storage_protocol_version: StorageProtocolVersion) -> Self {
        let l2_system_upgrade_tx_hash = storage_protocol_version
            .upgrade_tx_hash
            .as_ref()
            .map(|hash| H256::from_slice(hash));
        api::ProtocolVersion {
            version_id: storage_protocol_version.id as u16,
            timestamp: storage_protocol_version.timestamp as u64,
            verification_keys_hashes: L1VerifierConfig {
                params: VerifierParams {
                    recursion_node_level_vk_hash: H256::from_slice(
                        &storage_protocol_version.recursion_node_level_vk_hash,
                    ),
                    recursion_leaf_level_vk_hash: H256::from_slice(
                        &storage_protocol_version.recursion_leaf_level_vk_hash,
                    ),
                    recursion_circuits_set_vks_hash: H256::from_slice(
                        &storage_protocol_version.recursion_circuits_set_vks_hash,
                    ),
                },
                recursion_scheduler_level_vk_hash: H256::from_slice(
                    &storage_protocol_version.recursion_scheduler_level_vk_hash,
                ),
            },
            base_system_contracts: BaseSystemContractsHashes {
                entrypoint: H256::from_slice(&storage_protocol_version.bootloader_code_hash),
                default_aa: H256::from_slice(&storage_protocol_version.default_account_code_hash),
            },
            l2_system_upgrade_tx_hash,
        }
    }
}
