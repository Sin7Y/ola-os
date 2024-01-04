use crate::{
    errors::ClientError,
    operation::{execute_contract::ExecuteContractBuilder, SyncTransactionHandle},
    signer::Signer,
    OlaTxSigner,
};
use ola_types::{
    api::{BlockIdVariant, BlockNumber},
    l2::L2Tx,
    request::TransactionRequest,
    tx::primitives::PackedEthSignature,
    Address, Bytes,
};
use ola_web3_decl::{
    jsonrpsee::http_client::{HttpClient, HttpClientBuilder},
    namespaces::{eth::EthNamespaceClient, ola::OlaNamespaceClient},
};

#[derive(Debug)]
pub struct Wallet<S, P> {
    pub provider: P,
    pub signer: Signer<S>,
}

impl<S> Wallet<S, HttpClient>
where
    S: OlaTxSigner,
{
    pub fn with_http_client(
        rpc_address: &str,
        signer: Signer<S>,
    ) -> Result<Wallet<S, HttpClient>, ClientError> {
        let client = HttpClientBuilder::default().build(rpc_address)?;

        Ok(Wallet {
            provider: client,
            signer,
        })
    }
}

impl<S, P> Wallet<S, P>
where
    S: OlaTxSigner,
    P: EthNamespaceClient + OlaNamespaceClient + Sync,
{
    pub fn new(provider: P, signer: Signer<S>) -> Self {
        Self { provider, signer }
    }

    pub fn address(&self) -> Address {
        self.signer.address
    }

    pub async fn get_nonce(&self) -> Result<u32, ClientError> {
        let nonce = self
            .provider
            .get_transaction_count(
                self.address(),
                Some(BlockIdVariant::BlockNumber(BlockNumber::Committed)),
            )
            .await?;

        Ok(nonce)
    }

    pub fn get_chain_id(&self) -> u16 {
        self.signer.chain_id.0
    }

    pub fn start_execute_contract(
        &self,
        from: Option<Address>,
        outer_signatures: Option<Vec<PackedEthSignature>>,
    ) -> ExecuteContractBuilder<'_, S, P> {
        ExecuteContractBuilder::new(self, from, outer_signatures)
    }

    pub fn create_tx_raw(&self, tx: L2Tx) -> Result<Bytes, ClientError> {
        let transaction_request: TransactionRequest = {
            let mut req: TransactionRequest = tx.clone().into();
            if let Some(meta) = req.eip712_meta.as_mut() {
                meta.custom_signature = Some(tx.common_data.signature);
            }
            req.from = Some(self.address());
            req
        };
        let signature = self
            .signer
            .ola_signer
            .sign_tx_request(transaction_request.clone())?;

        let encoded_tx = transaction_request.get_signed_bytes(&signature, self.signer.chain_id.0);
        Ok(Bytes(encoded_tx))
    }

    pub async fn send_transaction(
        &self,
        tx: L2Tx,
    ) -> Result<SyncTransactionHandle<'_, P>, ClientError> {
        let bytes = self.create_tx_raw(tx)?;
        let tx_hash = self.provider.send_raw_transaction(bytes).await?;
        Ok(SyncTransactionHandle::new(tx_hash, &self.provider))
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use ethereum_types::H256;
    use ola_lang_abi::Value;
    use ola_types::l2::L2Tx;
    use ola_types::request::TransactionRequest;
    use ola_types::tx::primitives::PackedEthSignature;
    use ola_types::{Address, L2ChainId, Nonce};
    use ola_web3_decl::jsonrpsee::http_client::HttpClientBuilder;

    use crate::abi::create_invoke_calldata_with_abi_file;
    use crate::signer::Signer;
    use crate::utils::{h256_to_u64_array, h512_to_u64_array};
    use crate::{key_store::OlaKeyPair, private_key_signer::PrivateKeySigner};

    use super::Wallet;

    use secp256k1::ecdsa::Signature;
    use secp256k1::{Error as Secp256k1Error, SecretKey};
    use secp256k1::{Message, PublicKey, Secp256k1};

    #[test]
    fn test_ecdsa() {
        let key_pair = OlaKeyPair::from_random();
        let public_key = key_pair.public.clone();
        let public_key_string = hex::encode(&public_key);
        let (x, y) = split_pubkey(public_key.0.as_slice()).unwrap();
        let x_u256 = h256_to_u64_array(H256(x)).unwrap();
        let y_u256 = h256_to_u64_array(H256(y)).unwrap();
        let x_hex = hex::encode(x);
        let y_hex = hex::encode(y);

        println!("private key: {}", key_pair.private_key_str());
        println!("public key: {}", public_key_string);
        println!("public key x: {}, y: P{}", x_hex, y_hex);
        println!("public key x fields: {:?}", x_u256);
        println!("public key y fields: {:?}", y_u256);
        println!("address: {}", key_pair.address_str());

        let message = H256::random();
        println!("message: {}", hex::encode(message));

        let signature = PackedEthSignature::sign_raw(&key_pair.secret, &message).unwrap();
        let r = signature.r();
        let s = signature.s();

        let mut r_h256 = [0u8; 32];
        r_h256.copy_from_slice(r);
        let mut s_h256 = [0u8; 32];
        s_h256.copy_from_slice(s);
        println!("r: {}", hex::encode(r_h256));
        println!("s: {}", hex::encode(s_h256));
        let r_u256 = h256_to_u64_array(H256(r_h256)).unwrap();
        let s_u256 = h256_to_u64_array(H256(s_h256)).unwrap();
        println!("r fields: {:?}", r_u256);
        println!("s fields: {:?}", s_u256);

        let verify_result = verify_signature(&x, &y, &message.0, &r_h256, &s_h256).unwrap();
        println!("verify result: {}", verify_result);
        assert!(verify_result)
    }

    fn split_pubkey(pubkey: &[u8]) -> Option<([u8; 32], [u8; 32])> {
        if pubkey.len() != 64 {
            return None;
        }
        let mut x = [0u8; 32];
        let mut y = [0u8; 32];
        x.copy_from_slice(&pubkey[0..32]);
        y.copy_from_slice(&pubkey[32..64]);

        Some((x, y))
    }

    fn verify_signature(
        pub_x: &[u8; 32],
        pub_y: &[u8; 32],
        msg: &[u8],
        sig_r: &[u8; 32],
        sig_s: &[u8; 32],
    ) -> Result<bool, Secp256k1Error> {
        let secp = Secp256k1::new();

        let mut pub_key_bytes = [0u8; 65];
        pub_key_bytes[0] = 4;
        pub_key_bytes[1..33].copy_from_slice(pub_x);
        pub_key_bytes[33..].copy_from_slice(pub_y);
        let public_key = PublicKey::from_slice(&pub_key_bytes)?;

        let message = Message::from_slice(msg)?;

        let mut signature_bytes = [0u8; 64];
        signature_bytes[..32].copy_from_slice(sig_r);
        signature_bytes[32..].copy_from_slice(sig_s);
        let signature = Signature::from_compact(&signature_bytes)?;

        Ok(secp.verify_ecdsa(&message, &signature, &public_key).is_ok())
    }

    #[tokio::test]
    async fn test_send_transaction() {
        let eth_private_key = H256::random();
        let key_pair = OlaKeyPair::from_etherum_private_key(eth_private_key).unwrap();
        let pk_signer = PrivateKeySigner::new(key_pair.clone());
        let signer = Signer::new(pk_signer, key_pair.address, L2ChainId(270));
        let client = HttpClientBuilder::default()
            .build("http://localhost:13000")
            .unwrap();

        let wallet = Wallet::new(client, signer);
        let nonce = 0;

        println!("from: {}", wallet.address());
        let contract_address = H256([
            0u8, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0,
            0, 0, 0, 0,
        ]);
        let abi_file =
            File::open("examples/vote_simple_abi.json").expect("failed to open ABI file");
        let function_sig = "vote_proposal(u32)";
        let params = vec![Value::U32(1)];
        let calldata = create_invoke_calldata_with_abi_file(
            abi_file,
            function_sig,
            params,
            &wallet.address(),
            &contract_address,
            None,
        )
        .unwrap();
        println!("{:?}", calldata);

        // let l2Tx: L2Tx = wallet
        //     .start_execute_contract(None)
        //     .calldata(calldata)
        //     .contract_address(contract_address)
        //     .nonce(Nonce(nonce))
        //     .tx()
        //     .await
        //     .unwrap();
        // let bytes = wallet.create_tx_raw(l2Tx.clone()).unwrap();
        // let b = bytes.0.as_slice();
        // let (decoded_transaction, _) = TransactionRequest::from_bytes(b, 270).unwrap();
        // let origin_transaction: TransactionRequest = l2Tx.into();

        // assert_eq!(origin_transaction, decoded_transaction)

        // let handle = wallet
        //     .start_execute_contract(None)
        //     .calldata(calldata)
        //     .contract_address(contract_address)
        //     .nonce(Nonce(nonce))
        //     .send()
        //     .await;
        // match handle {
        //     Ok(_) => println!("ok"),
        //     Err(e) => println!("err: {:?}", e),
        // }
    }
}
