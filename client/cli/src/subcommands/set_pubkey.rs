use std::path::PathBuf;

use anyhow::{bail, Ok, Result};
use clap::Parser;
use ola_types::{L2ChainId, Nonce};
use ola_wallet_sdk::{
    abi::create_set_public_key_calldata, key_store::OlaKeyPair,
    private_key_signer::PrivateKeySigner, provider::ProviderParams, signer::Signer, wallet::Wallet,
};
use ola_web3_decl::jsonrpsee::http_client::HttpClientBuilder;

#[derive(Debug, Parser)]
pub struct SetPubKey {
    #[clap(long, help = "network name, can be local or pre-alpha")]
    network: Option<String>,
    #[clap(long, help = "Provide transaction nonce manually")]
    nonce: Option<u32>,
    #[clap(long, env = "OLA_KEYSTORE", help = "Path to keystore config JSON file")]
    keystore: String,
}

impl SetPubKey {
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

        let keystore_path = PathBuf::from(self.keystore);
        if !keystore_path.exists() {
            anyhow::bail!("keystore file not found");
        }
        let password = rpassword::prompt_password("Enter password: ")?;
        let key_pair = OlaKeyPair::from_keystore(keystore_path, &password)?;
        let public_key = key_pair.public;

        let pk_signer = PrivateKeySigner::new(key_pair.clone());
        let signer = Signer::new(pk_signer, key_pair.address, L2ChainId(network.chain_id));
        let client = HttpClientBuilder::default()
            .build(network.http_endpoint.as_str())
            .unwrap();
        let wallet = Wallet::new(client, signer);

        let from = key_pair.address;
        let nonce = if let Some(n) = self.nonce {
            n
        } else {
            wallet.get_addr_nonce(from).await.unwrap()
        };
        let calldata = create_set_public_key_calldata(&from, public_key)?;
        let handle = wallet
            .start_execute_contract(Some(from), None)
            .calldata(calldata)
            .nonce(Nonce(nonce))
            .send()
            .await?;
        let tx_hash = hex::encode(&handle.hash());
        println!("tx_hash: {}", tx_hash);

        Ok(())
    }
}
