use std::{fs::File, path::PathBuf};

use anyhow::{bail, Ok, Result};
use clap::Parser;
use ola_lang_abi::{Abi, Param, Value};
use ola_types::{L2ChainId, Nonce};
use ola_wallet_sdk::{
    abi::create_calldata, key_store::OlaKeyPair, parser::ToValue,
    private_key_signer::PrivateKeySigner, provider::ProviderParams, signer::Signer,
    utils::h256_from_hex_be, wallet::Wallet,
};
use ola_web3_decl::jsonrpsee::http_client::HttpClientBuilder;

use crate::{path::ExpandedPathbufParser, utils::from_hex_be};

#[derive(Debug, Parser)]
pub struct Invoke {
    #[clap(long, help = "network name, can be local or pre-alpha")]
    network: Option<String>,
    #[clap(long, help = "AA Address")]
    aa: Option<String>,
    #[clap(long, help = "Provide transaction nonce manually")]
    nonce: Option<u32>,
    #[clap(long, env = "OLA_KEYSTORE", help = "Path to keystore config JSON file")]
    keystore: String,
    #[clap(
        value_parser = ExpandedPathbufParser,
        help = "Path to the JSON keystore"
    )]
    abi: PathBuf,
    #[clap(help = "One or more contract calls. See documentation for more details")]
    calls: Vec<String>,
}

impl Invoke {
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

        let mut arg_iter = self.calls.into_iter();
        let contract_address_hex = arg_iter.next().expect("contract address needed");
        let contract_address =
            from_hex_be(contract_address_hex.as_str()).expect("invalid contract address");

        let abi_file = File::open(self.abi).expect("failed to open ABI file");
        let function_sig_name = arg_iter.next().expect("function signature needed");
        let abi: Abi = serde_json::from_reader(abi_file)?;
        let func = abi
            .functions
            .iter()
            .find(|func| func.name == function_sig_name)
            .expect("function not found");
        let func_inputs = &func.inputs;
        if arg_iter.len() != func_inputs.len() {
            bail!(
                "invalid args length: {} args expected, you input {}",
                func_inputs.len(),
                arg_iter.len()
            )
        }
        let param_to_input: Vec<(&Param, String)> =
            func_inputs.into_iter().zip(arg_iter.into_iter()).collect();
        let params: Vec<Value> = param_to_input
            .iter()
            .map(|(p, i)| ToValue::parse_input((**p).clone(), i.clone()))
            .collect();

        let pk_signer = PrivateKeySigner::new(key_pair.clone());
        let signer = Signer::new(pk_signer, key_pair.address, L2ChainId(network.chain_id));
        let client = HttpClientBuilder::default()
            .build(network.http_endpoint.as_str())
            .unwrap();
        let wallet = Wallet::new(client, signer);

        let from = if let Some(addr) = self.aa {
            h256_from_hex_be(addr.as_str()).unwrap()
        } else {
            key_pair.address
        };
        let nonce = if let Some(n) = self.nonce {
            n
        } else {
            wallet.get_addr_nonce(from).await.unwrap()
        };
        dbg!(nonce);

        let calldata = create_calldata(
            &abi,
            func.signature().as_str(),
            params,
            &from,
            &contract_address,
            None,
        )?;

        let handle = wallet
            .start_execute_contract(Some(from), None)
            .calldata(calldata)
            .nonce(Nonce(nonce))
            .send()
            .await?;
        let tx_hash = hex::encode(&handle.hash());
        println!("tx_hash: 0x{}", tx_hash);

        Ok(())
    }
}
