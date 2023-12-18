use ola_types::{
    l2::L2Tx, request::PaymasterParams, tx::primitives::PackedEthSignature, Address, Nonce,
};
use ola_web3_decl::namespaces::ola::OlaNamespaceClient;

use crate::{errors::ClientError, wallet::Wallet, EthereumSigner};

use super::SyncTransactionHandle;

pub struct ExecuteContractBuilder<'a, S: EthereumSigner, P> {
    wallet: &'a Wallet<S, P>,
    contract_address: Option<Address>,
    calldata: Option<Vec<u8>>,
    nonce: Option<Nonce>,
    factory_deps: Option<Vec<Vec<u8>>>,
    paymaster_params: Option<PaymasterParams>,
    outer_signatures: Option<Vec<PackedEthSignature>>,
}

impl<'a, S, P> ExecuteContractBuilder<'a, S, P>
where
    S: EthereumSigner,
    P: OlaNamespaceClient + Sync,
{
    pub fn new(
        wallet: &'a Wallet<S, P>,
        outer_signatures: Option<Vec<PackedEthSignature>>,
    ) -> Self {
        Self {
            wallet,
            contract_address: None,
            calldata: None,
            nonce: None,
            factory_deps: None,
            paymaster_params: None,
            outer_signatures,
        }
    }

    pub async fn tx(self) -> Result<L2Tx, ClientError> {
        let paymaster_params = self.paymaster_params.clone().unwrap_or_default();

        let contract_address = self
            .contract_address
            .ok_or_else(|| ClientError::MissingRequiredField("contract_address".into()))?;

        let calldata = self
            .calldata
            .ok_or_else(|| ClientError::MissingRequiredField("calldata".into()))?;

        let nonce = match self.nonce {
            Some(nonce) => nonce,
            None => Nonce(self.wallet.get_nonce().await?),
        };

        let signature = self
            .wallet
            .signer
            .sign_execute_contract(
                contract_address,
                calldata.clone(),
                nonce,
                self.factory_deps.clone(),
                paymaster_params.clone(),
            )
            .await
            .map_err(ClientError::SigningError)?;

        let mut execute_contract = L2Tx::new(
            contract_address,
            calldata,
            nonce,
            self.wallet.signer.eth_signer.get_address().await?,
            self.factory_deps,
            paymaster_params,
        );

        let signatures = match self.outer_signatures {
            Some(mut signatures) => {
                signatures.push(signature);
                signatures
            }
            None => vec![signature],
        };
        execute_contract.set_signature(signatures);
        Ok(execute_contract)
    }

    pub async fn send(self) -> Result<SyncTransactionHandle<'a, P>, ClientError> {
        let wallet = self.wallet;
        let tx = self.tx().await?;

        wallet.send_transaction(tx).await
    }

    pub fn calldata(mut self, calldata: Vec<u8>) -> Self {
        self.calldata = Some(calldata);
        self
    }

    pub fn contract_address(mut self, address: Address) -> Self {
        self.contract_address = Some(address);
        self
    }

    pub fn nonce(mut self, nonce: Nonce) -> Self {
        self.nonce = Some(nonce);
        self
    }

    pub fn factory_deps(mut self, factory_deps: Vec<Vec<u8>>) -> Self {
        self.factory_deps = Some(factory_deps);
        self
    }

    pub fn paymaster_params(mut self, paymaster_params: PaymasterParams) -> Self {
        self.paymaster_params = Some(paymaster_params);
        self
    }
}