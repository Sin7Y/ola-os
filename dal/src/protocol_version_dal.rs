use ola_contracts::BaseSystemContracts;
use ola_types::{
    protocol_version::{ProtocolUpgradeTx, ProtocolVersionId},
    H256,
};

use crate::StorageProcessor;

#[derive(Debug)]
pub struct ProtocolVersionsDal<'a, 'c> {
    pub storage: &'a mut StorageProcessor<'c>,
}

impl ProtocolVersionsDal<'_, '_> {
    pub async fn base_system_contracts_by_timestamp(
        &mut self,
        current_timestamp: u64,
    ) -> (BaseSystemContracts, ProtocolVersionId) {
        let row = sqlx::query!(
            "SELECT bootloader_code_hash, default_account_code_hash, id FROM protocol_versions
                WHERE timestamp <= $1
                ORDER BY id DESC
                LIMIT 1
            ",
            current_timestamp as i64
        )
        .fetch_one(self.storage.conn())
        .await
        .unwrap();
        let contracts = self
            .storage
            .storage_dal()
            .get_base_system_contracts(
                H256::from_slice(&row.bootloader_code_hash),
                H256::from_slice(&row.default_account_code_hash),
            )
            .await;
        (contracts, (row.id as u16).try_into().unwrap())
    }

    pub async fn get_protocol_upgrade_tx(
        &mut self,
        protocol_version_id: ProtocolVersionId,
    ) -> Option<ProtocolUpgradeTx> {
        let row = sqlx::query!(
            "
                SELECT upgrade_tx_hash FROM protocol_versions
                WHERE id = $1
            ",
            protocol_version_id as i32
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()?;
        if let Some(hash) = row.upgrade_tx_hash {
            Some(
                self.storage
                    .transactions_dal()
                    .get_tx_by_hash(H256::from_slice(&hash))
                    .await
                    .unwrap_or_else(|| {
                        panic!(
                            "Missing upgrade tx for protocol version {}",
                            protocol_version_id as u16
                        );
                    })
                    .try_into()
                    .unwrap(),
            )
        } else {
            None
        }
    }
}
