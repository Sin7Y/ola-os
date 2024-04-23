use bigdecimal::BigDecimal;
use ola_types::{api, L1BatchNumber, L2ChainId, MiniblockNumber, H256, U256, U64};
use ola_utils::bigdecimal_to_u256;
use sqlx::Row;

use crate::models::storage_block::{
    web3_block_where_sql, StorageBlockDetails, StorageL1BatchDetails,
};
use crate::models::storage_transaction::{extract_web3_transaction, web3_transaction_select_sql};
use crate::{
    models::storage_block::{bind_block_where_sql_params, web3_block_number_to_sql},
    SqlxError, StorageProcessor,
};

use ola_constants::blocks::EMPTY_UNCLES_HASH;

const BLOCK_GAS_LIMIT: u32 = u32::MAX;

#[derive(Debug)]
pub struct BlocksWeb3Dal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl BlocksWeb3Dal<'_, '_> {
    pub async fn get_block_by_web3_block_id(
        &mut self,
        block_id: api::BlockId,
        include_full_transactions: bool,
        chain_id: L2ChainId,
    ) -> sqlx::Result<Option<api::Block<api::TransactionVariant>>> {
        let transactions_sql = if include_full_transactions {
            web3_transaction_select_sql()
        } else {
            "transactions.hash as tx_hash"
        };

        let query = format!(
            "SELECT
                miniblocks.hash as block_hash,
                miniblocks.number,
                miniblocks.l1_batch_number,
                miniblocks.timestamp,
                miniblocks.base_fee_per_gas,
                prev_miniblock.hash as parent_hash,
                l1_batches.timestamp as l1_batch_timestamp,
                transactions.gas_limit as gas_limit,
                transactions.refunded_gas as refunded_gas,
                {}
            FROM miniblocks
            LEFT JOIN miniblocks prev_miniblock
                ON prev_miniblock.number = miniblocks.number - 1
            LEFT JOIN l1_batches
                ON l1_batches.number = miniblocks.l1_batch_number
            LEFT JOIN transactions
                ON transactions.miniblock_number = miniblocks.number
            WHERE {}
            ORDER BY transactions.index_in_block ASC",
            transactions_sql,
            web3_block_where_sql(block_id, 1)
        );

        let query = bind_block_where_sql_params(&block_id, sqlx::query(&query));
        let rows = query.fetch_all(self.storage.conn()).await?.into_iter();

        let block = rows.fold(None, |prev_block, db_row| {
            let mut block = prev_block.unwrap_or_else(|| {
                // This code will be only executed for the first row in the DB response.
                // All other rows will only be used to extract relevant transactions.
                let hash = db_row
                    .try_get("block_hash")
                    .map_or_else(|_| H256::zero(), H256::from_slice);
                let number = U64::from(db_row.get::<i64, &str>("number"));
                let l1_batch_number = db_row
                    .try_get::<i64, &str>("l1_batch_number")
                    .map(U64::from)
                    .ok();
                let l1_batch_timestamp = db_row
                    .try_get::<i64, &str>("l1_batch_timestamp")
                    .map(U256::from)
                    .ok();
                let parent_hash = db_row
                    .try_get("parent_hash")
                    .map_or_else(|_| H256::zero(), H256::from_slice);
                let base_fee_per_gas = db_row.get::<BigDecimal, &str>("base_fee_per_gas");

                api::Block {
                    hash,
                    parent_hash,
                    uncles_hash: EMPTY_UNCLES_HASH,
                    number,
                    l1_batch_number,
                    gas_limit: BLOCK_GAS_LIMIT.into(),
                    base_fee_per_gas: bigdecimal_to_u256(base_fee_per_gas),
                    timestamp: db_row.get::<i64, &str>("timestamp").into(),
                    l1_batch_timestamp,
                    // TODO: include logs
                    ..api::Block::default()
                }
            });
            if db_row.try_get::<&[u8], &str>("tx_hash").is_ok() {
                let tx_gas_limit = bigdecimal_to_u256(db_row.get::<BigDecimal, &str>("gas_limit"));
                let tx_refunded_gas = U256::from((db_row.get::<i64, &str>("refunded_gas")) as u32);

                block.gas_used += tx_gas_limit - tx_refunded_gas;
                let tx = if include_full_transactions {
                    let tx = extract_web3_transaction(db_row, chain_id);
                    api::TransactionVariant::Full(tx)
                } else {
                    api::TransactionVariant::Hash(H256::from_slice(db_row.get("tx_hash")))
                };
                block.transactions.push(tx);
            }
            Some(block)
        });
        Ok(block)
    }

    pub async fn get_block_tx_count(
        &mut self,
        block_id: api::BlockId,
    ) -> sqlx::Result<Option<(MiniblockNumber, U256)>> {
        let query = format!(
            "SELECT number, l1_tx_count + l2_tx_count AS tx_count FROM miniblocks WHERE {}",
            web3_block_where_sql(block_id, 1)
        );
        let query = bind_block_where_sql_params(&block_id, sqlx::query(&query));

        Ok(query.fetch_optional(self.storage.conn()).await?.map(|row| {
            let miniblock_number = row.get::<i64, _>("number") as u32;
            let tx_count = row.get::<i32, _>("tx_count") as u32;
            (MiniblockNumber(miniblock_number), tx_count.into())
        }))
    }

    pub async fn get_miniblock_hash(
        &mut self,
        block_number: MiniblockNumber,
    ) -> sqlx::Result<Option<H256>> {
        let hash = sqlx::query!(
            r#"
            SELECT
                hash
            FROM
                miniblocks
            WHERE
                number = $1
            "#,
            block_number.0 as i64
        )
        .fetch_optional(self.storage.conn())
        .await?
        .map(|row| H256::from_slice(&row.hash));
        Ok(hash)
    }

    pub async fn get_l1_batch_number_of_miniblock(
        &mut self,
        miniblock_number: MiniblockNumber,
    ) -> sqlx::Result<Option<L1BatchNumber>> {
        let number: Option<i64> = sqlx::query!(
            r#"
            SELECT
                l1_batch_number
            FROM
                miniblocks
            WHERE
                number = $1
            "#,
            miniblock_number.0 as i64
        )
        .fetch_optional(self.storage.conn())
        .await?
        .and_then(|row| row.l1_batch_number);

        Ok(number.map(|number| L1BatchNumber(number as u32)))
    }

    pub async fn get_miniblock_range_of_l1_batch(
        &mut self,
        l1_batch_number: L1BatchNumber,
    ) -> sqlx::Result<Option<(MiniblockNumber, MiniblockNumber)>> {
        let row = sqlx::query!(
            r#"
            SELECT
                MIN(miniblocks.number) AS "min?",
                MAX(miniblocks.number) AS "max?"
            FROM
                miniblocks
            WHERE
                l1_batch_number = $1
            "#,
            l1_batch_number.0 as i64
        )
        .fetch_one(self.storage.conn())
        .await?;

        Ok(match (row.min, row.max) {
            (Some(min), Some(max)) => {
                Some((MiniblockNumber(min as u32), MiniblockNumber(max as u32)))
            }
            (None, None) => None,
            _ => unreachable!(),
        })
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
            .map(|n| MiniblockNumber(n as u32));
        Ok(block_number)
    }

    pub async fn get_block_details(
        &mut self,
        block_number: MiniblockNumber,
    ) -> sqlx::Result<Option<api::BlockDetails>> {
        let res = sqlx::query_as!(
            StorageBlockDetails,
            r#"
            SELECT 
                number,
                timestamp,
                hash,
                l1_tx_count,
                l2_tx_count,
                bootloader_code_hash,
                default_aa_code_hash
            FROM 
                miniblocks
            WHERE 
                number = $1
            "#,
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
                default_aa_code_hash
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
