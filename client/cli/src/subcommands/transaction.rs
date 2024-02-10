use anyhow::{bail, Ok, Result};
use clap::Parser;
use ola_types::api::TransactionDetails;
use ola_wallet_sdk::provider::{ExtendProvider, ProviderParams};

use crate::utils::from_hex_be;

#[derive(Debug, Parser)]
pub struct Transaction {
    #[clap(long, help = "network name, can be local or pre-alpha")]
    network: Option<String>,
    #[clap(help = "Transaction hash")]
    hash: String,
}

impl Transaction {
    pub async fn run(self) -> Result<()> {
        let network = if let Some(network) = self.network {
            match network.as_str() {
                "local" => ProviderParams::local(),
                "pre-alpha" => ProviderParams::pre_alpha(),
                _ => {
                    bail!("invalid network name")
                }
            }
        } else {
            ProviderParams::pre_alpha()
        };
        let hash = from_hex_be(self.hash.as_str()).expect("invalid transaction hash");
        let provider = ExtendProvider::with_http_client(network.http_endpoint.as_str()).unwrap();
        let tx_detail = provider.get_transaction_detail(hash).await?;
        match tx_detail {
            Some(tx) => {
                let tx_json = serde_json::to_string(&tx).unwrap();
                println!("Transaction Details:\n{}", tx_json);
            }
            None => {
                println!("No transaction found by tx_hash: {}", self.hash)
            }
        }
        Ok(())
    }
}
