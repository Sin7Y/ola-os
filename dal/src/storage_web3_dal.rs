use std::ops;

use ola_types::{H256, MiniblockNumber};

use crate::{StorageProcessor, SqlxError};

#[derive(Debug)]
pub struct StorageWeb3Dal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl StorageWeb3Dal<'_, '_> {
    // pub async fn modified_keys_in_miniblocks(
    //     &mut self,
    //     miniblock_numbers: ops::RangeInclusive<MiniblockNumber>,
    // ) -> Vec<H256> {
    //     sqlx::query!(
    //         "SELECT DISTINCT hashed_key FROM storage_logs WHERE miniblock_number BETWEEN $1 and $2",
    //         miniblock_numbers.start().0 as i64,
    //         miniblock_numbers.end().0 as i64,
    //     )
    //     .fetch_all(self.storage.conn())
    //     .await
    //     .unwrap()
    //     .into_iter()
    //     .map(|row| H256::from_slice(&row.hashed_key))
    //     .collect()
    // }
}