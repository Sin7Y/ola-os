use ola_types::api;
use sqlx::{postgres::PgArguments, query::Query, Postgres};

pub fn web3_block_number_to_sql(block_number: api::BlockNumber) -> String {
    match block_number {
        api::BlockNumber::Number(number) => number.to_string(),
        api::BlockNumber::Earliest => 0.to_string(),
        api::BlockNumber::Pending => {
            "(SELECT (MAX(number) + 1) as number FROM miniblocks)".to_string()
        }
        api::BlockNumber::Latest | api::BlockNumber::Committed => {
            "(SELECT MAX(number) as number FROM miniblocks)".to_string()
        }
        api::BlockNumber::Finalized => "
                (SELECT COALESCE(
                    (
                        SELECT MAX(number) FROM miniblocks
                        WHERE l1_batch_number = (
                            SELECT MAX(number) FROM l1_batches
                            JOIN eth_txs ON
                                l1_batches.eth_execute_tx_id = eth_txs.id
                            WHERE
                                eth_txs.confirmed_eth_tx_history_id IS NOT NULL
                        )
                    ),
                    0
                ) as number)
            "
        .to_string(),
    }
}

pub fn bind_block_where_sql_params<'q>(
    block_id: &'q api::BlockId,
    query: Query<'q, Postgres, PgArguments>,
) -> Query<'q, Postgres, PgArguments> {
    match block_id {
        // these block_id types result in `$1` in the query string, which we have to `bind`
        api::BlockId::Hash(block_hash) => query.bind(block_hash.as_bytes()),
        api::BlockId::Number(api::BlockNumber::Number(number)) => {
            query.bind(number.as_u64() as i64)
        }
        // others don't introduce `$1`, so we don't have to `bind` anything
        _ => query,
    }
}
