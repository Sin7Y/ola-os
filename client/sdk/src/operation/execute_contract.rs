use ethereum_types::H256;
use ola_types::{
    l2::L2Tx, request::PaymasterParams, tx::primitives::PackedEthSignature, Address, Nonce,
};
use ola_web3_decl::namespaces::ola::OlaNamespaceClient;

use crate::OlaTxSigner;
use crate::{errors::ClientError, wallet::Wallet};

use super::SyncTransactionHandle;

pub struct ExecuteContractBuilder<'a, S: OlaTxSigner, P> {
    wallet: &'a Wallet<S, P>,
    from: Option<Address>,
    contract_address: Option<Address>,
    calldata: Option<Vec<u8>>,
    nonce: Option<Nonce>,
    factory_deps: Option<Vec<Vec<u8>>>,
    paymaster_params: Option<PaymasterParams>,
    outer_signatures: Option<Vec<PackedEthSignature>>,
}

impl<'a, S, P> ExecuteContractBuilder<'a, S, P>
where
    S: OlaTxSigner,
    P: OlaNamespaceClient + Sync,
{
    pub fn new(
        wallet: &'a Wallet<S, P>,
        from: Option<Address>,
        outer_signatures: Option<Vec<PackedEthSignature>>,
    ) -> Self {
        Self {
            wallet,
            from,
            contract_address: Some(H256([
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x80, 0x01,
            ])),
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
        let from = self
            .from
            .unwrap_or_else(|| self.wallet.signer.ola_signer.get_address().unwrap());

        let signature = self
            .wallet
            .signer
            .sign_execute_contract(
                self.wallet.get_chain_id(),
                Some(from),
                contract_address,
                calldata.clone(),
                nonce,
                self.factory_deps.clone(),
                paymaster_params.clone(),
            )
            .map_err(ClientError::SigningError)?;

        let mut execute_contract = L2Tx::new(
            contract_address,
            calldata,
            nonce,
            self.wallet.signer.ola_signer.get_address()?,
            self.factory_deps,
            paymaster_params,
        );

        let signatures = match self.outer_signatures {
            Some(mut signatures) => {
                signatures.insert(0, signature);
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
        // println!("tx: {:?}", tx);
        wallet.send_transaction(tx).await
    }

    pub fn calldata(mut self, calldata: Vec<u8>) -> Self {
        self.calldata = Some(calldata);
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
