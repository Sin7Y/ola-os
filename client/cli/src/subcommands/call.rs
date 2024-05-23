use std::{fs::File, path::PathBuf};

use anyhow::{bail, Ok, Result};
use clap::Parser;
use ola_lang_abi::{Abi, Param, Value};
use ola_utils::convert::bytes_to_u64s;
use ola_wallet_sdk::{
    abi::build_call_request,
    key_store::OlaKeyPair,
    parser::{FromValue, ToValue},
    provider::{ExtendProvider, ProviderParams},
    utils::h256_from_hex_be,
};

use crate::{path::ExpandedPathbufParser, utils::from_hex_be};

#[derive(Debug, Parser)]
pub struct Call {
    #[clap(long, help = "network name, can be local or alpha")]
    network: Option<String>,
    #[clap(long, help = "AA Address")]
    aa: Option<String>,
    #[clap(
        value_parser = ExpandedPathbufParser,
        help = "Path to the JSON keystore"
    )]
    abi: PathBuf,
    #[clap(help = "One or more contract calls. See documentation for more details")]
    calls: Vec<String>,
}

impl Call {
    pub async fn run(self) -> Result<()> {
        let network = if let Some(network) = self.network {
            match network.as_str() {
                "local" => ProviderParams::local(),
                "alpha" => ProviderParams::alpha(),
                _ => {
                    bail!("invalid network name")
                }
            }
        } else {
            ProviderParams::alpha()
        };

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

        let provider = ExtendProvider::with_http_client(network.http_endpoint.as_str()).unwrap();

        let from = if let Some(addr) = self.aa {
            h256_from_hex_be(addr.as_str()).unwrap()
        } else {
            OlaKeyPair::from_random().address
        };

        let call_request = build_call_request(
            &abi,
            func.signature().as_str(),
            params,
            &from,
            &contract_address,
        )?;

        let bytes_ret: Vec<u8> = provider.call_transaction(call_request).await?.0;
        let u64_ret = bytes_to_u64s(bytes_ret.clone());
        let decoded = abi
            .decode_output_from_slice(func.signature().as_str(), &u64_ret)
            .unwrap();
        println!("Return data:");
        for dp in decoded.1.reader().by_index {
            let value = FromValue::parse_input(dp.value.clone());
            println!("{}", value);
        }
        Ok(())
    }
}
