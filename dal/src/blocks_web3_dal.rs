use ola_types::{api, L1BatchNumber, MiniblockNumber, H256, U256, U64};
use sqlx::types::BigDecimal;
use sqlx::Row;

use crate::models::storage_block::StorageL1BatchDetails;
use crate::{
    models::storage_block::{bind_block_where_sql_params, web3_block_number_to_sql},
    SqlxError, StorageProcessor,
};

#[derive(Debug)]
pub struct BlocksWeb3Dal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl BlocksWeb3Dal<'_, '_> {
    pub async fn resolve_block_id(
        &mut self,
        block_id: api::BlockId,
    ) -> Result<Option<MiniblockNumber>, SqlxError> {
        let query_string = match block_id {
            api::BlockId::Hash(_) => "SELECT number FROM miniblocks WHERE hash = $1".to_owned(),
            api::BlockId::Number(api::BlockNumber::Number(_)) => {
                // The reason why instead of returning the `block_number` directly we use query is
                // to handle numbers of blocks that are not created yet.
                // the `SELECT number FROM miniblocks WHERE number=block_number` for
                // non-existing block number will returns zero.
                "SELECT number FROM miniblocks WHERE number = $1".to_owned()
            }
            api::BlockId::Number(api::BlockNumber::Earliest) => {
                return Ok(Some(MiniblockNumber(0)));
            }
            api::BlockId::Number(block_number) => web3_block_number_to_sql(block_number),
        };
        let row = bind_block_where_sql_params(&block_id, sqlx::query(&query_string))
            .fetch_optional(self.storage.conn())
            .await?;

        let block_number = row
            .and_then(|row| row.get::<Option<i64>, &str>("number"))
            .map(|n| MiniblockNumber(n as u32));
        Ok(block_number)
    }

    pub async fn get_block_details(
        &mut self,
        block_number: MiniblockNumber,
    ) -> sqlx::Result<Option<api::BlockDetails>> {
        let res = sqlx::query_as!(
            StorageL1BatchDetails,
            "SELECT number, timestamp, hash, l1_tx_count, l2_tx_count, \
                        bootloader_code_hash, default_aa_code_hash, protocol_version \
                    FROM miniblocks \
                    WHERE number = $1",
            block_number.0 as i64,
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(Into::into);

        Ok(res)
    }

    pub async fn get_l1_batch_details(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) -> sqlx::Result<Option<api::L1BatchDetails>> {
        let res = sqlx::query_as!(
            StorageL1BatchDetails,
            r#"
            SELECT
                number,
                timestamp,      
                l1_tx_count,
                l2_tx_count,
                hash,
                bootloader_code_hash,
                default_aa_code_hash,
            FROM 
                l1_batches
            WHERE 
                number = $1
            "#,
            l1_batch_number.0 as i64
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(Into::into);

        Ok(res)
    }
}
