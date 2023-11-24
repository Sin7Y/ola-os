use crate::StorageProcessor;

#[derive(Debug)]
pub struct TokensDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

// impl TokensDal<'_, '_> {
//     pub async fn get_all_l2_token_addresses(&mut self) -> Vec<Address> {
//         {
//             let records = sqlx::query!("SELECT l2_address FROM tokens")
//                 .fetch_all(self.storage.conn())
//                 .await
//                 .unwrap();
//             let addresses: Vec<Address> = records
//                 .into_iter()
//                 .map(|record| Address::from_slice(&record.l2_address))
//                 .collect();
//             addresses
//         }
//     }

//     pub async fn add_tokens(&mut self, tokens: Vec<TokenInfo>) {
//         {
//             let mut copy = self
//             .storage
//             .conn()
//             .copy_in_raw(
//                 "COPY tokens (l1_address, l2_address, name, symbol, decimals, well_known, created_at, updated_at)
//                 FROM STDIN WITH (DELIMITER '|')",
//             )
//             .await
//             .unwrap();

//             let mut bytes: Vec<u8> = Vec::new();
//             let now = Utc::now().naive_utc().to_string();
//             for TokenInfo {
//                 l1_address,
//                 l2_address,
//                 metadata:
//                     TokenMetadata {
//                         name,
//                         symbol,
//                         decimals,
//                     },
//             } in tokens
//             {
//                 let l1_address_str = format!("\\\\x{}", hex::encode(l1_address.0));
//                 let l2_address_str = format!("\\\\x{}", hex::encode(l2_address.0));
//                 let row = format!(
//                     "{}|{}|{}|{}|{}|FALSE|{}|{}\n",
//                     l1_address_str, l2_address_str, name, symbol, decimals, now, now
//                 );
//                 bytes.extend_from_slice(row.as_bytes());
//             }
//             copy.send(bytes).await.unwrap();
//             copy.finish().await.unwrap();
//         }
//     }
// }
