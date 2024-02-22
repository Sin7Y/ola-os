use std::collections::HashMap;

use ola_types::{
    block::{L1BatchHeader, MiniblockHeader},
    commitment::L1BatchMetadata,
    protocol_version::ProtocolVersionId,
    L1BatchNumber, MiniblockNumber, H256, U256,
};

use crate::{
    models::storage_block::{StorageL1Batch, StorageL1BatchHeader, StorageMiniblockHeader},
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
        olaos_logs::info!("is_genesis_needed count = {}", count);
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

    pub async fn insert_l1_batch(
        &mut self,
        header: &L1BatchHeader,
        initial_bootloader_contents: &[(usize, U256)],
    ) {
        let initial_bootloader_contents = serde_json::to_value(initial_bootloader_contents)
            .expect("failed to serialize initial_bootloader_contents to JSON value");
        let used_contract_hashes = serde_json::to_value(&header.used_contract_hashes)
            .expect("failed to serialize used_contract_hashes to JSON value");

        sqlx::query!(
            "INSERT INTO l1_batches (\
                number, l1_tx_count, l2_tx_count, timestamp, is_finished, \
                initial_bootloader_heap_content, used_contract_hashes, \
                bootloader_code_hash, default_aa_code_hash, protocol_version, \
                created_at, updated_at \
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), now())",
            header.number.0 as i64,
            header.l1_tx_count as i32,
            header.l2_tx_count as i32,
            header.timestamp as i64,
            header.is_finished,
            initial_bootloader_contents,
            used_contract_hashes,
            header.base_system_contracts_hashes.entrypoint.as_bytes(),
            header.base_system_contracts_hashes.default_aa.as_bytes(),
            header.protocol_version.map(|v| v as i32),
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }

    pub async fn insert_miniblock(&mut self, miniblock_header: &MiniblockHeader) {
        sqlx::query!(
            "INSERT INTO miniblocks ( \
                number, timestamp, hash, l1_tx_count, l2_tx_count, \
                bootloader_code_hash, default_aa_code_hash, protocol_version, \
                created_at, updated_at \
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now(), now())",
            miniblock_header.number.0 as i64,
            miniblock_header.timestamp as i64,
            miniblock_header.hash.as_bytes(),
            miniblock_header.l1_tx_count as i32,
            miniblock_header.l2_tx_count as i32,
            miniblock_header
                .base_system_contracts_hashes
                .entrypoint
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

    pub async fn mark_miniblocks_as_executed_in_l1_batch(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) {
        sqlx::query!(
            "UPDATE miniblocks \
            SET l1_batch_number = $1 \
            WHERE l1_batch_number IS NULL",
            l1_batch_number.0 as i32,
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
        let last_l1_batch = sqlx::query_as!(
            StorageL1BatchHeader,
            "SELECT number, l1_tx_count, l2_tx_count, \
                timestamp, is_finished, used_contract_hashes, \
                bootloader_code_hash, default_aa_code_hash, protocol_version \
            FROM l1_batches \
            ORDER BY number DESC \
            LIMIT 1"
        )
        .fetch_one(self.storage.conn())
        .await
        .unwrap();

        last_l1_batch.into()
    }

    // pub async fn get_l1_batch_metadata(
    //     &mut self,
    //     number: L1BatchNumber,
    // ) -> anyhow::Result<Option<L1BatchWithMetadata>> {
    //     let Some(l1_batch) = self
    //         .get_storage_l1_batch(number)
    //         .await
    //         .context("get_storage_l1_batch()")?
    //     else {
    //         return Ok(None);
    //     };
    //     self.get_l1_batch_with_metadata(l1_batch)
    //         .await
    //         .context("get_l1_batch_with_metadata")
    // }

    // pub async fn get_storage_l1_batch(
    //     &mut self,
    //     number: L1BatchNumber,
    // ) -> sqlx::Result<Option<StorageL1Batch>> {
    //     sqlx::query_as!(
    //         StorageL1Batch,
    //         r#"
    //         SELECT
    //             number,
    //             timestamp,
    //             is_finished,
    //             l1_tx_count,
    //             l2_tx_count,
    //             fee_account_address,
    //             bloom,
    //             priority_ops_onchain_data,
    //             hash,
    //             parent_hash,
    //             commitment,
    //             compressed_write_logs,
    //             compressed_contracts,
    //             eth_prove_tx_id,
    //             eth_commit_tx_id,
    //             eth_execute_tx_id,
    //             merkle_root_hash,
    //             l2_to_l1_logs,
    //             l2_to_l1_messages,
    //             used_contract_hashes,
    //             compressed_initial_writes,
    //             compressed_repeated_writes,
    //             l2_l1_compressed_messages,
    //             l2_l1_merkle_root,
    //             l1_gas_price,
    //             l2_fair_gas_price,
    //             rollup_last_leaf_index,
    //             zkporter_is_available,
    //             bootloader_code_hash,
    //             default_aa_code_hash,
    //             base_fee_per_gas,
    //             aux_data_hash,
    //             pass_through_data_hash,
    //             meta_parameters_hash,
    //             protocol_version,
    //             system_logs,
    //             compressed_state_diffs,
    //             events_queue_commitment,
    //             bootloader_initial_content_commitment,
    //             pubdata_input
    //         FROM
    //             l1_batches
    //             LEFT JOIN commitments ON commitments.l1_batch_number = l1_batches.number
    //         WHERE
    //             number = $1
    //         "#,
    //         number.0 as i64
    //     )
    //     .instrument("get_storage_l1_batch")
    //     .with_arg("number", &number)
    //     .fetch_optional(self.storage.conn())
    //     .await
    // }

    // #[tracing::instrument(name = "get_sealed_l1_batch_number", skip_all)]
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

    #[tracing::instrument(name = "get_l1_batch_header", skip_all)]
    pub async fn get_l1_batch_header(&mut self, number: L1BatchNumber) -> Option<L1BatchHeader> {
        sqlx::query_as!(
            StorageL1BatchHeader,
            "SELECT number, l1_tx_count, l2_tx_count, \
                timestamp, is_finished, used_contract_hashes, \
                bootloader_code_hash, default_aa_code_hash, protocol_version \
            FROM l1_batches \
            WHERE number = $1",
            number.0 as i64
        )
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
                bootloader_code_hash = $7, rollup_last_leaf_index = $8, \
                aux_data_hash = $9, pass_through_data_hash = $10, meta_parameters_hash = $11, \
                updated_at = now() \
            WHERE number = $12",
            metadata.root_hash.as_bytes(),
            metadata.merkle_root_hash.as_bytes(),
            metadata.commitment.as_bytes(),
            metadata.block_meta_params.default_aa_code_hash.as_bytes(),
            metadata.repeated_writes_compressed,
            metadata.initial_writes_compressed,
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

    pub async fn get_last_l1_batch_number_with_metadata(&mut self) -> L1BatchNumber {
        let number =
            sqlx::query!("SELECT MAX(number) as \"number\" FROM l1_batches WHERE hash IS NOT NULL")
                .fetch_one(self.storage.conn())
                .await
                .unwrap()
                .number
                .expect("DAL invocation before genesis");

        L1BatchNumber(number as u32)
    }

    pub async fn save_l1_batch_metadata(
        &mut self,
        block_number: L1BatchNumber,
        block_metadata: L1BatchMetadata,
        previous_root_hash: H256,
    ) {
        let update_result = sqlx::query!(
            "
                UPDATE l1_batches SET
                    hash = $1, merkle_root_hash = $2, commitment = $3, 
                    compressed_repeated_writes = $4, compressed_initial_writes = $5,
                    parent_hash = $6, rollup_last_leaf_index = $7, 
                    aux_data_hash = $8, pass_through_data_hash = $9, meta_parameters_hash = $10,
                    updated_at = NOW()
                WHERE number = $11 AND hash IS NULL
            ",
            block_metadata.root_hash.as_bytes(),
            block_metadata.merkle_root_hash.as_bytes(),
            block_metadata.commitment.as_bytes(),
            block_metadata.repeated_writes_compressed,
            block_metadata.initial_writes_compressed,
            previous_root_hash.0.to_vec(),
            block_metadata.rollup_last_leaf_index as i64,
            block_metadata.aux_data_hash.as_bytes(),
            block_metadata.pass_through_data_hash.as_bytes(),
            block_metadata.meta_parameters_hash.as_bytes(),
            block_number.0 as i64,
        )
        .execute(self.storage.conn())
        .await
        .unwrap();

        if update_result.rows_affected() == 0 {
            olaos_logs::info!(
                "L1 batch {} info wasn't updated. Details: root_hash: {:?}, merkle_root_hash: {:?}, parent_hash: {:?}, commitment: {:?}",
                block_number.0 as i64,
                block_metadata.root_hash.0.to_vec(),
                block_metadata.merkle_root_hash.0.to_vec(),
                previous_root_hash.0.to_vec(),
                block_metadata.commitment.0.to_vec(),
            );

            // block was already processed. Verify that existing hashes match
            let matched: i64 = sqlx::query!(
                r#"
                    SELECT COUNT(*) as "count!"
                    FROM l1_batches
                    WHERE number = $1
                        AND hash = $2
                        AND merkle_root_hash = $3
                        AND parent_hash = $4
                "#,
                block_number.0 as i64,
                block_metadata.root_hash.0.to_vec(),
                block_metadata.merkle_root_hash.0.to_vec(),
                previous_root_hash.0.to_vec(),
            )
            .fetch_one(self.storage.conn())
            .await
            .unwrap()
            .count;

            assert_eq!(matched, 1, "Root hash verification failed. Hashes for some of previously processed blocks do not match");
        }
    }
}
