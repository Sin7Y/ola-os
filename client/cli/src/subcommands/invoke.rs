use std::{fs::File, io::Read, path::PathBuf};

use anyhow::{bail, Ok, Result};
use clap::Parser;
use colored::Colorize;
use ola_lang_abi::{Abi, FixedArray4, Param, Type, Value};
use ola_types::{L2ChainId, Nonce};
use ola_wallet_sdk::{
    abi::create_invoke_calldata_with_abi,
    key_store::OlaKeyPair,
    private_key_signer::PrivateKeySigner,
    provider::ProviderParams,
    signer::Signer,
    utils::{h256_from_hex_be, h256_to_u64_array, OLA_FIELD_ORDER},
    wallet::{self, Wallet},
};
use ola_web3_decl::jsonrpsee::http_client::HttpClientBuilder;

use crate::{path::ExpandedPathbufParser, utils::from_hex_be};

// let from = key_pair.address;
//         let ola_http_endpoint = "https://testnet.ola.network";
//         let nonce = 0;
//         let chain_id = 270;

#[derive(Debug, Parser)]
pub struct Invoke {
    #[clap(long, help = "network name")]
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
                "test" => ProviderParams::pub_test(),
                _ => {
                    bail!("invalid network name")
                }
            }
        } else {
            ProviderParams::pub_test()
        };

        let keystore_path = PathBuf::from(self.keystore);
        if !keystore_path.exists() {
            anyhow::bail!("keystore file not found");
        }
        let password = rpassword::prompt_password("Enter password: ")?;
        let key_pair = OlaKeyPair::from_keystore(keystore_path, &password)?;

        let unexpected_end_of_args = || anyhow::anyhow!("unexpected end of arguments");
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
            .map(|(p, i)| Self::parse_input(p.clone().clone(), i.clone()))
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

        let calldata = create_invoke_calldata_with_abi(
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
            .contract_address(contract_address)
            .nonce(Nonce(nonce))
            .send()
            .await?;
        let tx_hash = hex::encode(&handle.hash());
        println!("tx_hash: {}", tx_hash);

        Ok(())
    }

    fn parse_input(param: Param, input: String) -> Value {
        let parse_result = match param.type_ {
            ola_lang_abi::Type::U32 => Self::parse_u32(input),
            ola_lang_abi::Type::Field => Self::parse_field(input),
            ola_lang_abi::Type::Hash => Self::parse_hash(input),
            ola_lang_abi::Type::Address => Self::parse_address(input),
            ola_lang_abi::Type::Bool => Self::parse_bool(input),
            ola_lang_abi::Type::FixedArray(t, size) => Self::parse_fixed_array(*t, size, input),
            ola_lang_abi::Type::String => Self::parse_string(input),
            ola_lang_abi::Type::Fields => Self::parse_fields(input),
            ola_lang_abi::Type::Array(t) => Self::parse_array(*t, input),
            ola_lang_abi::Type::Tuple(attrs) => Self::parse_tuple(attrs, input),
        };
        parse_result.unwrap()
    }

    fn parse_u32(input: String) -> Result<Value> {
        let value = input.parse::<u32>().expect("invalid u32 input");
        Ok(Value::U32(value as u64))
    }

    fn parse_field(input: String) -> Result<Value> {
        let value = input.parse::<u64>().expect("invalid field element input");
        if value > OLA_FIELD_ORDER {
            bail!("invalid field element input")
        }
        Ok(Value::Field(value))
    }

    fn parse_hash(input: String) -> Result<Value> {
        let hash = from_hex_be(input.as_str()).expect("invalid contract address");
        let u256 = h256_to_u64_array(hash)?;
        Ok(Value::Hash(FixedArray4(u256)))
    }

    fn parse_address(input: String) -> Result<Value> {
        Self::parse_hash(input)
    }

    fn parse_bool(input: String) -> Result<Value> {
        let value = input.parse::<bool>().expect("invalid bool input");
        Ok(Value::Bool(value))
    }

    fn parse_fixed_array(t: Type, size: u64, input: String) -> Result<Value> {
        match t {
            Type::U32
            | Type::Field
            | Type::Hash
            | Type::Address
            | Type::Bool
            | Type::String
            | Type::Fields => {
                let s = input.as_str();
                if !s.starts_with('[') || !s.ends_with(']') {
                    bail!("invalid fixed array format.")
                }
                let content = &s[1..s.len() - 1];
                let split_content: Vec<String> =
                    content.split(',').map(|s| s.to_string()).collect();
                if split_content.len() as u64 != size {
                    bail!("invalid fixed array size")
                }
                let items: Vec<Value> = split_content
                    .iter()
                    .map(|i| {
                        Self::parse_input(
                            Param {
                                name: "tmp".to_string(),
                                type_: t.clone(),
                            },
                            i.clone(),
                        )
                    })
                    .collect();
                Ok(Value::FixedArray(items, t))
            }
            Type::FixedArray(_, _) | Type::Array(_) | Type::Tuple(_) => {
                bail!("Composite types in FixedArray has not been supported for cli tools.")
            }
        }
    }

    fn parse_string(input: String) -> Result<Value> {
        Ok(Value::String(input))
    }

    fn parse_fields(input: String) -> Result<Value> {
        let s = input.as_str();
        if !s.starts_with('[') || !s.ends_with(']') {
            bail!("invalid fixed array format.")
        }
        let content = &s[1..s.len() - 1];
        let split_content: Vec<String> = content.split(',').map(|s| s.to_string()).collect();
        let items: Vec<u64> = split_content
            .iter()
            .map(|i| {
                let value = i.parse::<u64>().expect("invalid field element input");
                if value > OLA_FIELD_ORDER {
                    panic!("invalid field element input")
                }
                value
            })
            .collect();
        Ok(Value::Fields(items))
    }

    fn parse_array(t: Type, input: String) -> Result<Value> {
        match t {
            Type::U32
            | Type::Field
            | Type::Hash
            | Type::Address
            | Type::Bool
            | Type::String
            | Type::Fields => {
                let s = input.as_str();
                if !s.starts_with('[') || !s.ends_with(']') {
                    bail!("invalid array format.")
                }
                let content = &s[1..s.len() - 1];
                let split_content: Vec<String> =
                    content.split(',').map(|s| s.to_string()).collect();
                let items: Vec<Value> = split_content
                    .iter()
                    .map(|i| {
                        Self::parse_input(
                            Param {
                                name: "tmp".to_string(),
                                type_: t.clone(),
                            },
                            i.clone(),
                        )
                    })
                    .collect();
                Ok(Value::Array(items, t))
            }
            Type::FixedArray(_, _) | Type::Array(_) | Type::Tuple(_) => {
                bail!("Composite types in Array has not been supported for cli tools.")
            }
        }
    }

    fn parse_tuple(attrs: Vec<(String, Type)>, input: String) -> Result<Value> {
        let s = input.as_str();
        if !s.starts_with('{') || !s.ends_with('}') {
            bail!("invalid tuple format.")
        }
        let content = &s[1..s.len() - 1];
        let split_content: Vec<String> = content.split(',').map(|s| s.to_string()).collect();
        if split_content.len() != attrs.len() {
            bail!("invalid tuple size")
        }
        let items: Vec<(String, Value)> = split_content
            .iter()
            .zip(attrs.iter())
            .map(|(i, (name, t))| {
                match t {
                    Type::U32
                    | Type::Field
                    | Type::Hash
                    | Type::Address
                    | Type::Bool
                    | Type::String
                    | Type::Fields => {}
                    Type::FixedArray(_, _) | Type::Array(_) | Type::Tuple(_) => {
                        panic!("Composite types in Tuple has not been supported for cli tools.")
                    }
                }
                let v = Self::parse_input(
                    Param {
                        name: name.clone(),
                        type_: t.clone(),
                    },
                    i.clone(),
                );
                (name.clone(), v)
            })
            .collect();
        Ok(Value::Tuple(items))
    }
}
