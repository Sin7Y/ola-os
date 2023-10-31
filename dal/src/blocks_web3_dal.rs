use ola_types::{api, MiniblockNumber};
use sqlx::Row;

use crate::{
    models::storage_block::{bind_block_where_sql_params, web3_block_number_to_sql},
    SqlxError, StorageProcessor,
};

#[derive(Debug)]
pub struct BlocksWeb3Dal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl BlocksWeb3Dal<'_, '_> {
    #[tracing::instrument(name = "get_sealed_block_number", skip_all)]
    pub async fn get_sealed_miniblock_number(&mut self) -> Result<MiniblockNumber, SqlxError> {
        let number = sqlx::query!("SELECT MAX(number) as \"number\" FROM miniblocks")
            .fetch_one(self.storage.conn())
            .await?
            .number
            .expect("DAL invocation before genesis");
        Ok(MiniblockNumber(number as u64))
    }

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
            .map(|n| MiniblockNumber(n as u64));
        Ok(block_number)
    }
}
