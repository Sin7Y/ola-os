use crate::models::storage_protocol_version::StorageProtocolVersion;
use ola_contracts::BaseSystemContracts;
use ola_types::protocol_version::{ProtocolUpgradeTx, ProtocolVersion, ProtocolVersionId};
use ola_types::H256;

use crate::StorageProcessor;

#[derive(Debug)]
pub struct ProtocolVersionsDal<'a, 'c> {
    pub storage: &'a mut StorageProcessor<'c>,
}

impl ProtocolVersionsDal<'_, '_> {
    pub async fn save_protocol_version(&mut self, version: ProtocolVersion) {
        let tx_hash = version
            .tx
            .as_ref()
            .map(|tx| tx.common_data.hash().0.to_vec());

        let mut db_transaction = self.storage.start_transaction().await;
        if let Some(tx) = version.tx {
            db_transaction
                .transactions_dal()
                .insert_system_transaction(tx)
                .await;
        }

        sqlx::query!(
            "INSERT INTO protocol_versions
                    (id, timestamp, bootloader_code_hash,
                        default_account_code_hash, upgrade_tx_hash, created_at)
                VALUES ($1, $2, $3, $4, $5, now())
                ",
            version.id as i32,
            version.timestamp as i64,
            version.base_system_contracts_hashes.entrypoint.as_bytes(),
            version.base_system_contracts_hashes.default_aa.as_bytes(),
            tx_hash
        )
        .execute(db_transaction.conn())
        .await
        .unwrap();

        db_transaction.commit().await;
    }

    pub async fn base_system_contracts_by_timestamp(
        &mut self,
        current_timestamp: i64,
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
