use std::{fs::File, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use ethereum_types::{H256, U256};
use ola_lang_abi::{Abi, FixedArray4, Value};
use ola_types::{L2ChainId, Nonce};
use ola_wallet_sdk::{
    abi::create_invoke_calldata_with_abi,
    key_store::OlaKeyPair,
    private_key_signer::PrivateKeySigner,
    program_meta::ProgramMeta,
    provider::ProviderParams,
    signer::Signer,
    utils::{h256_from_hex_be, h256_to_u64_array, is_h256_a_valid_ola_hash},
    wallet::Wallet,
};
use ola_web3_decl::jsonrpsee::http_client::HttpClientBuilder;

use crate::path::ExpandedPathbufParser;

// pub const CONTRACT_DEPLOYER_ADDRESS: Address = H256([
//     0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
//     0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x05,
// ]);

#[derive(Debug, Parser)]
pub struct Deploy {
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
        help = "Path to contract binary file"
    )]
    contract: PathBuf,
}

impl Deploy {
    pub async fn run(self) -> Result<()> {
        let network = if let Some(network) = self.network {
            match network.as_str() {
                "local" => ProviderParams::local(),
                "test" => ProviderParams::pub_test(),
                _ => {
                    anyhow::bail!("invalid network name")
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
        let prog_meta = ProgramMeta::from_file(self.contract)?;

        let deployer_abi_str = include_str!("../abi/ContractDeployer.json");
        let deployer_abi: Abi = serde_json::from_str(deployer_abi_str)?;
        let func = deployer_abi
            .functions
            .iter()
            .find(|func| func.name == "create2")
            .expect("create2 function not found in abi file");

        let salt = Self::random_salt();
        let prog_hash = prog_meta.program_hash;
        let bytecode_hash = prog_meta.bytecode_hash;
        let code = prog_meta.instructions;

        let params = [
            Value::Hash(FixedArray4(salt.0)),
            Value::Hash(FixedArray4(h256_to_u64_array(prog_hash).unwrap())),
            Value::Hash(FixedArray4(h256_to_u64_array(bytecode_hash).unwrap())),
        ];

        let from = if let Some(addr) = self.aa {
            h256_from_hex_be(addr.as_str()).unwrap()
        } else {
            key_pair.address
        };

        let pk_signer = PrivateKeySigner::new(key_pair.clone());
        let signer = Signer::new(pk_signer, key_pair.address, L2ChainId(network.chain_id));
        let client = HttpClientBuilder::default()
            .build(network.http_endpoint.as_str())
            .unwrap();
        let wallet = Wallet::new(client, signer);

        let nonce = if let Some(n) = self.nonce {
            n
        } else {
            wallet.get_addr_nonce(from).await.unwrap()
        };

        let contract_address = H256([
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x80, 0x05,
        ]);

        let calldata = create_invoke_calldata_with_abi(
            &deployer_abi,
            func.signature().as_str(),
            params.to_vec(),
            &from,
            &contract_address,
            Some(code),
        )?;

        let handle = wallet
            .start_deploy_contract(Some(from))
            .calldata(calldata)
            .nonce(Nonce(nonce))
            .raw_code(prog_meta.bytes)
            .send()
            .await?;
        let tx_hash = hex::encode(&handle.hash());
        println!("tx_hash: {}", tx_hash);
        Ok(())
    }

    fn random_salt() -> U256 {
        let mut salt = H256::random();
        while !is_h256_a_valid_ola_hash(salt) {
            salt = H256::random();
        }
        U256(h256_to_u64_array(salt).unwrap())
    }
}
