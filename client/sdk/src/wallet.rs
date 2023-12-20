use crate::{
    errors::ClientError,
    operation::{execute_contract::ExecuteContractBuilder, SyncTransactionHandle},
    signer::Signer,
    EthereumSigner,
};
use ola_types::{
    api::{BlockIdVariant, BlockNumber},
    l2::L2Tx,
    tx::primitives::PackedEthSignature,
    Address,
};
use ola_web3_decl::{
    jsonrpsee::http_client::{HttpClient, HttpClientBuilder},
    namespaces::{eth::EthNamespaceClient, ola::OlaNamespaceClient},
};

#[derive(Debug)]
pub struct Wallet<S: EthereumSigner, P> {
    pub provider: P,
    pub signer: Signer<S>,
}

impl<S> Wallet<S, HttpClient>
where
    S: EthereumSigner,
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
    S: EthereumSigner,
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

    pub fn start_execute_contract(
        &self,
        outer_signatures: Option<Vec<PackedEthSignature>>,
    ) -> ExecuteContractBuilder<'_, S, P> {
        ExecuteContractBuilder::new(self, outer_signatures)
    }

    pub async fn send_transaction(
        &self,
        tx: L2Tx,
    ) -> Result<SyncTransactionHandle<'_, P>, ClientError> {
        // todo
        Err(ClientError::MissingRequiredField("todo".to_string()))
    }
}
