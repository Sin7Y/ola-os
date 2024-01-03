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
    use ola_types::{Address, L2ChainId, Nonce};
    use ola_web3_decl::jsonrpsee::http_client::HttpClientBuilder;

    use crate::abi::create_invoke_calldata_with_abi_file;
    use crate::signer::Signer;
    use crate::{key_store::OlaKeyPair, private_key_signer::PrivateKeySigner};

    use super::Wallet;

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
