use std::collections::HashMap;

use bigdecimal::{BigDecimal, FromPrimitive};
use ola_config::constants::ethereum::MAX_GAS_PER_PUBDATA_BYTE;
use ola_types::{
    block::{L1BatchHeader, MiniblockHeader},
    protocol_version::ProtocolVersionId,
    L1BatchNumber, MiniblockNumber, H256,
};

use crate::{
    models::storage_block::{StorageL1BatchHeader, StorageMiniblockHeader},
    StorageProcessor,
};

#[derive(Debug)]
pub struct BlocksDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl BlocksDal<'_, '_> {
    pub async fn is_genesis_needed(&mut self) -> bool {
        let count = sqlx::query!("SELECT COUNT(*) as \"count!\" FROM l1_batches")
            .fetch_one(self.storage.conn())
            .await
            .unwrap()
            .count;
        count == 0
    }

    pub async fn get_l1_batch_state_root(&mut self, number: L1BatchNumber) -> Option<H256> {
        sqlx::query!(
            "SELECT hash FROM l1_batches WHERE number = $1",
            number.0 as i64
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .and_then(|row| row.hash)
        .map(|hash| H256::from_slice(&hash))
    }

    pub async fn insert_miniblock(&mut self, miniblock_header: &MiniblockHeader) {
        sqlx::query!(
            "INSERT INTO miniblocks ( \
                number, timestamp, hash, l1_tx_count, l2_tx_count, \
                base_fee_per_gas, l1_gas_price, l2_fair_gas_price, gas_per_pubdata_limit, \
                bootloader_code_hash, default_aa_code_hash, protocol_version, \
                created_at, updated_at \
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, now(), now())",
            miniblock_header.number.0 as i64,
            miniblock_header.timestamp as i64,
            miniblock_header.hash.as_bytes(),
            miniblock_header.l1_tx_count as i32,
            miniblock_header.l2_tx_count as i32,
            BigDecimal::from_u32(0),
            0,
            0,
            MAX_GAS_PER_PUBDATA_BYTE as i64,
            miniblock_header
                .base_system_contracts_hashes
                .bootloader
                .as_bytes(),
            miniblock_header
                .base_system_contracts_hashes
                .default_aa
                .as_bytes(),
            miniblock_header.protocol_version.map(|v| v as i32),
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }

    #[tracing::instrument(name = "get_sealed_miniblock_number", skip_all)]
    pub async fn get_sealed_miniblock_number(&mut self) -> MiniblockNumber {
        let number: i64 = sqlx::query!("SELECT MAX(number) as \"number\" FROM miniblocks")
            .fetch_one(self.storage.conn())
            .await
            .unwrap()
            .number
            .unwrap_or(0);
        MiniblockNumber(number as u32)
    }

    #[tracing::instrument(name = "get_newest_l1_batch_header", skip_all)]
    pub async fn get_newest_l1_batch_header(&mut self) -> L1BatchHeader {
        // TODO: remove price
        let last_l1_batch = sqlx::query_as!(
            StorageL1BatchHeader,
            "SELECT number, l1_tx_count, l2_tx_count, \
                timestamp, is_finished, fee_account_address, l2_to_l1_logs, l2_to_l1_messages, \
                bloom, priority_ops_onchain_data, \
                used_contract_hashes, base_fee_per_gas, l1_gas_price, \
                l2_fair_gas_price, bootloader_code_hash, default_aa_code_hash, protocol_version \
            FROM l1_batches \
            ORDER BY number DESC \
            LIMIT 1"
        )
        .fetch_one(self.storage.conn())
        .await
        .unwrap();

        last_l1_batch.into()
    }

    #[tracing::instrument(name = "get_sealed_l1_batch_number", skip_all)]
    pub async fn get_sealed_l1_batch_number(&mut self) -> L1BatchNumber {
        let number = sqlx::query!(
            "SELECT MAX(number) as \"number\" FROM l1_batches WHERE is_finished = TRUE"
        )
        .fetch_one(self.storage.conn())
        .await
        .unwrap()
        .number
        .expect("DAL invocation before genesis");

        L1BatchNumber(number as u32)
    }

    pub async fn get_l1_batch_factory_deps(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) -> HashMap<H256, Vec<u8>> {
        sqlx::query!(
            "SELECT bytecode_hash, bytecode FROM factory_deps \
            INNER JOIN miniblocks ON miniblocks.number = factory_deps.miniblock_number \
            WHERE miniblocks.l1_batch_number = $1",
            l1_batch_number.0 as i64
        )
        .fetch_all(self.storage.conn())
        .await
        .unwrap()
        .into_iter()
        .map(|row| (H256::from_slice(&row.bytecode_hash), row.bytecode))
        .collect()
    }

    pub async fn get_miniblock_range_of_l1_batch(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) -> Option<(MiniblockNumber, MiniblockNumber)> {
        let row = sqlx::query!(
            "SELECT MIN(miniblocks.number) as \"min?\", MAX(miniblocks.number) as \"max?\" \
            FROM miniblocks \
            WHERE l1_batch_number = $1",
            l1_batch_number.0 as i64
        )
        .fetch_one(self.storage.conn())
        .await
        .unwrap();

        Some((
            MiniblockNumber(row.min? as u32),
            MiniblockNumber(row.max? as u32),
        ))
    }

    pub async fn get_miniblock_header(
        &mut self,
        miniblock_number: MiniblockNumber,
    ) -> Option<MiniblockHeader> {
        sqlx::query_as!(
            StorageMiniblockHeader,
            "SELECT number, timestamp, hash, l1_tx_count, l2_tx_count, \
                base_fee_per_gas, l1_gas_price, l2_fair_gas_price, \
                bootloader_code_hash, default_aa_code_hash, protocol_version \
            FROM miniblocks \
            WHERE number = $1",
            miniblock_number.0 as i64,
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(Into::into)
    }

    pub async fn get_l1_batch_state_root_and_timestamp(
        &mut self,
        number: L1BatchNumber,
    ) -> Option<(H256, u64)> {
        let row = sqlx::query!(
            "SELECT timestamp, hash FROM l1_batches WHERE number = $1",
            number.0 as i64
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()?;

        Some((H256::from_slice(&row.hash?), row.timestamp as u64))
    }

    pub async fn get_miniblock_timestamp(
        &mut self,
        miniblock_number: MiniblockNumber,
    ) -> Option<u64> {
        sqlx::query!(
            "SELECT timestamp FROM miniblocks WHERE number = $1",
            miniblock_number.0 as i64,
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(|row| row.timestamp as u64)
    }

    pub async fn get_batch_protocol_version_id(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) -> Option<ProtocolVersionId> {
        {
            let row = sqlx::query!(
                "SELECT protocol_version FROM l1_batches WHERE number = $1",
                l1_batch_number.0 as i64
            )
            .fetch_optional(self.storage.conn())
            .await
            .unwrap()?;
            row.protocol_version.map(|v| (v as u16).try_into().unwrap())
        }
    }

    pub async fn get_l1_batch_header(&mut self, number: L1BatchNumber) -> Option<L1BatchHeader> {
        sqlx::query_as!(
            StorageL1BatchHeader,
            "SELECT number, l1_tx_count, l2_tx_count, \
                timestamp, is_finished, fee_account_address, l2_to_l1_logs, l2_to_l1_messages, \
                bloom, priority_ops_onchain_data, \
                used_contract_hashes, base_fee_per_gas, l1_gas_price, \
                l2_fair_gas_price, bootloader_code_hash, default_aa_code_hash, protocol_version \
            FROM l1_batches \
            WHERE number = $1",
            number.0 as i64
        )
        .instrument("get_l1_batch_header")
        .with_arg("number", &number)
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(Into::into)
    }

    pub async fn save_genesis_l1_batch_metadata(&mut self, metadata: &L1BatchMetadata) {
        sqlx::query!(
            "UPDATE l1_batches \
            SET hash = $1, merkle_root_hash = $2, commitment = $3, default_aa_code_hash = $4, \
                compressed_repeated_writes = $5, compressed_initial_writes = $6, \
                l2_l1_compressed_messages = $7, l2_l1_merkle_root = $8, \
                zkporter_is_available = $9, bootloader_code_hash = $10, rollup_last_leaf_index = $11, \
                aux_data_hash = $12, pass_through_data_hash = $13, meta_parameters_hash = $14, \
                updated_at = now() \
            WHERE number = $15",
            metadata.root_hash.as_bytes(),
            metadata.merkle_root_hash.as_bytes(),
            metadata.commitment.as_bytes(),
            metadata.block_meta_params.default_aa_code_hash.as_bytes(),
            metadata.repeated_writes_compressed,
            metadata.initial_writes_compressed,
            metadata.l2_l1_messages_compressed,
            metadata.l2_l1_merkle_root.as_bytes(),
            metadata.block_meta_params.zkporter_is_available,
            metadata.block_meta_params.bootloader_code_hash.as_bytes(),
            metadata.rollup_last_leaf_index as i64,
            metadata.aux_data_hash.as_bytes(),
            metadata.pass_through_data_hash.as_bytes(),
            metadata.meta_parameters_hash.as_bytes(),
            0,
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }
}
