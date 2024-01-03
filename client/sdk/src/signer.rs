use ethereum_types::U64;
use ola_types::{
    l2::{L2Tx, TransactionType},
    request::{Eip712Meta, PaymasterParams, TransactionRequest},
    tx::primitives::PackedEthSignature,
    Address, Bytes, L2ChainId, Nonce,
};

use crate::{errors::SignerError, OlaTxSigner};

fn signing_failed_error(err: impl ToString) -> SignerError {
    SignerError::SigningFailed(err.to_string())
}
#[derive(Debug)]
pub struct Signer<S> {
    pub(crate) ola_signer: S,
    pub(crate) address: Address,
    pub(crate) chain_id: L2ChainId,
}

impl<S: OlaTxSigner> Signer<S> {
    pub fn new(ola_signer: S, address: Address, chain_id: L2ChainId) -> Self {
        Self {
            ola_signer,
            address,
            chain_id,
        }
    }

    pub fn sign_transaction(&self, transaction: &L2Tx) -> Result<PackedEthSignature, SignerError> {
        let transaction_request: TransactionRequest = transaction.clone().into();
        self.ola_signer
            .sign_tx_request(transaction_request)
            .map_err(signing_failed_error)
    }

    pub fn sign_transaction_request(
        &self,
        transaction_request: TransactionRequest,
    ) -> Result<PackedEthSignature, SignerError> {
        self.ola_signer
            .sign_tx_request(transaction_request)
            .map_err(signing_failed_error)
    }

    pub fn sign_execute_contract(
        &self,
        chain_id: u16,
        from: Option<Address>,
        contract: Address,
        calldata: Vec<u8>,
        nonce: Nonce,
        factory_deps: Option<Vec<Vec<u8>>>,
        paymaster_params: PaymasterParams,
    ) -> Result<PackedEthSignature, SignerError> {
        let initiator = match from {
            Some(from) => from,
            None => self.ola_signer.get_address()?,
        };
        // let execute_contract = L2Tx::new(
        //     contract,
        //     calldata,
        //     nonce,
        //     initiator,
        //     factory_deps,
        //     paymaster_params,
        // );

        let mut req = TransactionRequest {
            nonce: nonce.0.into(),
            from,
            to: Some(contract),
            input: Bytes(calldata),
            v: None,
            r: None,
            s: None,
            raw: None,
            transaction_type: Some(U64::from(TransactionType::OlaRawTransaction as u32)),
            eip712_meta: Some(Eip712Meta {
                factory_deps,
                custom_signature: None,
                paymaster_params: None,
            }),
            chain_id: Some(chain_id),
        };

        let signature = self
            .sign_transaction_request(req)
            .map_err(signing_failed_error)?;
        Ok(signature)
    }
}
