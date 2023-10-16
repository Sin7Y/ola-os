use ola_types::Address;

use crate::StorageProcessor;

#[derive(Debug)]
pub struct TokensDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl TokensDal<'_, '_> {
    pub async fn get_all_l2_token_addresses(&mut self) -> Vec<Address> {
        {
            let records = sqlx::query!("SELECT l2_address FROM tokens")
                .fetch_all(self.storage.conn())
                .await
                .unwrap();
            let addresses: Vec<Address> = records
                .into_iter()
                .map(|record| Address::from_slice(&record.l2_address))
                .collect();
            addresses
        }
    }
}
