use std::ops;

use ola_types::{
    get_nonce_key, storage::StorageKey, Address, L1BatchNumber, MiniblockNumber, H256, U256,
};
use ola_utils::convert::h256_to_u256;

use crate::{models::storage_block::ResolvedL1BatchForMiniblock, SqlxError, StorageProcessor};

#[derive(Debug)]
pub struct StorageWeb3Dal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl StorageWeb3Dal<'_, '_> {
    pub async fn get_address_historical_nonce(
        &mut self,
        address: Address,
        block_number: MiniblockNumber,
    ) -> Result<U256, SqlxError> {
        let nonce_key = get_nonce_key(&address);
        let nonce_value = self
            .get_historical_value_unchecked(&nonce_key, block_number)
            .await?;
        let full_nonce = h256_to_u256(nonce_value);
        Ok(full_nonce)
    }

    #[olaos_logs::instrument(name = "get_historical_value_unchecked", skip_all)]
    pub async fn get_historical_value_unchecked(
        &mut self,
        key: &StorageKey,
        block_number: MiniblockNumber,
    ) -> Result<H256, SqlxError> {
        {
            // We need to proper distinguish if the value is zero or None
            // for the VM to correctly determine initial writes.
            // So, we accept that the value is None if it's zero and it wasn't initially written at the moment.
            let hashed_key = key.hashed_key();

            sqlx::query!(
                r#"
                SELECT value
                FROM storage_logs
                WHERE storage_logs.hashed_key = $1 AND storage_logs.miniblock_number <= $2
                ORDER BY storage_logs.miniblock_number DESC, storage_logs.operation_number DESC
                LIMIT 1
                "#,
                hashed_key.as_bytes(),
                block_number.0 as i64
            )
            .fetch_optional(self.storage.conn())
            .await
            .map(|option_row| {
                option_row
                    .map(|row| H256::from_slice(&row.value))
                    .unwrap_or_else(H256::zero)
            })
        }
    }

    pub async fn modified_keys_in_miniblocks(
        &mut self,
        miniblock_numbers: ops::RangeInclusive<MiniblockNumber>,
    ) -> Vec<H256> {
        sqlx::query!(
            "SELECT DISTINCT hashed_key FROM storage_logs WHERE miniblock_number BETWEEN $1 and $2",
            miniblock_numbers.start().0 as i64,
            miniblock_numbers.end().0 as i64,
        )
        .fetch_all(self.storage.conn())
        .await
        .unwrap()
        .into_iter()
        .map(|row| H256::from_slice(&row.hashed_key))
        .collect()
    }

    /// Provides information about the L1 batch that the specified miniblock is a part of.
    /// Assumes that the miniblock is present in the DB; this is not checked, and if this is false,
    /// the returned value will be meaningless.
    pub async fn resolve_l1_batch_number_of_miniblock(
        &mut self,
        miniblock_number: MiniblockNumber,
    ) -> Result<ResolvedL1BatchForMiniblock, SqlxError> {
        let row = sqlx::query!(
            "SELECT \
                (SELECT l1_batch_number FROM miniblocks WHERE number = $1) as \"block_batch?\", \
                (SELECT MAX(number) + 1 FROM l1_batches) as \"max_batch?\"",
            miniblock_number.0 as i64
        )
        .fetch_one(self.storage.conn())
        .await?;

        Ok(ResolvedL1BatchForMiniblock {
            miniblock_l1_batch: row.block_batch.map(|n| L1BatchNumber(n as u32)),
            pending_l1_batch: L1BatchNumber(row.max_batch.unwrap_or(0) as u32),
        })
    }
}
