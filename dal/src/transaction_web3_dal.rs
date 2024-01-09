use ola_types::{
    api::{BlockId, BlockNumber},
    Address,
};
use sqlx::Error;

use crate::{SqlxError, StorageProcessor};

#[derive(Debug)]
pub struct TransactionsWeb3Dal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl TransactionsWeb3Dal<'_, '_> {
    pub async fn next_nonce_by_initiator_account(
        &mut self,
        initiator_address: Address,
    ) -> Result<u32, SqlxError> {
        let latest_block_number = self
            .storage
            .blocks_web3_dal()
            .resolve_block_id(BlockId::Number(BlockNumber::Latest))
            .await?
            .expect("Failed to get `latest` nonce");
        let latest_nonce = self
            .storage
            .storage_web3_dal()
            .get_address_historical_nonce(initiator_address, latest_block_number)
            .await?
            .as_u32();

        // Get nonces of non-rejected transactions, starting from the 'latest' nonce.
        // `latest` nonce is used, because it is guaranteed that there are no gaps before it.
        // `(miniblock_number IS NOT NULL OR error IS NULL)` is the condition that filters non-rejected transactions.
        // Query is fast because we have an index on (`initiator_address`, `nonce`)
        // and it cannot return more than `max_nonce_ahead` nonces.
        let non_rejected_nonces: Vec<u32> = sqlx::query!(
            r#"
            SELECT
                nonce AS "nonce!"
            FROM
                transactions
            WHERE
                initiator_address = $1
                AND nonce >= $2
                AND is_priority = FALSE
                AND (
                    miniblock_number IS NOT NULL
                    OR error IS NULL
                )
            ORDER BY
                nonce
            "#,
            initiator_address.as_bytes(),
            latest_nonce as i32
        )
        .fetch_all(self.storage.conn())
        .await?
        .into_iter()
        .map(|row| row.nonce as u32)
        .collect();

        // Find pending nonce as the first "gap" in nonces.
        let mut pending_nonce = latest_nonce;
        for nonce in non_rejected_nonces {
            if pending_nonce == nonce {
                pending_nonce += 1;
            } else {
                break;
            }
        }

        Ok(pending_nonce)
    }
}
