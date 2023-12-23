use ola_types::{
    l2::L2Tx,
    request::{PaymasterParams, TransactionRequest},
    tx::primitives::PackedEthSignature,
    Address, L2ChainId, Nonce,
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

    pub fn sign_execute_contract(
        &self,
        contract: Address,
        calldata: Vec<u8>,
        nonce: Nonce,
        factory_deps: Option<Vec<Vec<u8>>>,
        paymaster_params: PaymasterParams,
    ) -> Result<PackedEthSignature, SignerError> {
        self.sign_execute_contract_for_deploy(
            contract,
            calldata,
            nonce,
            factory_deps,
            paymaster_params,
        )
    }

    pub fn sign_execute_contract_for_deploy(
        &self,
        contract: Address,
        calldata: Vec<u8>,
        nonce: Nonce,
        factory_deps: Option<Vec<Vec<u8>>>,
        paymaster_params: PaymasterParams,
    ) -> Result<PackedEthSignature, SignerError> {
        let execute_contract = L2Tx::new(
            contract,
            calldata,
            nonce,
            self.ola_signer.get_address()?,
            factory_deps,
            paymaster_params,
        );

        let signature = self
            .sign_transaction(&execute_contract)
            .map_err(signing_failed_error)?;
        Ok(signature)
    }
}

// impl<S: EthereumSigner> Signer<S> {
//     pub fn new(eth_signer: S, address: Address, chain_id: L2ChainId) -> Self {
//         Self {
//             ola_signer: eth_signer,
//             address,
//             chain_id,
//         }
//     }

//     pub async fn sign_transaction(
//         &self,
//         transaction: &L2Tx,
//     ) -> Result<PackedEthSignature, SignerError> {
//         let domain = Eip712Domain::new(self.chain_id);
//         let transaction_request: TransactionRequest = transaction.clone().into();
//         self.ola_signer
//             .sign_typed_data(&domain, &transaction_request)
//             .await
//     }

//     pub async fn sign_execute_contract(
//         &self,
//         contract: Address,
//         calldata: Vec<u8>,
//         nonce: Nonce,
//         factory_deps: Option<Vec<Vec<u8>>>,
//         paymaster_params: PaymasterParams,
//     ) -> Result<PackedEthSignature, SignerError> {
//         self.sign_execute_contract_for_deploy(
//             contract,
//             calldata,
//             nonce,
//             factory_deps,
//             paymaster_params,
//         )
//         .await
//     }

//     pub async fn sign_execute_contract_for_deploy(
//         &self,
//         contract: Address,
//         calldata: Vec<u8>,
//         nonce: Nonce,
//         factory_deps: Option<Vec<Vec<u8>>>,
//         paymaster_params: PaymasterParams,
//     ) -> Result<PackedEthSignature, SignerError> {
//         let execute_contract = L2Tx::new(
//             contract,
//             calldata,
//             nonce,
//             self.ola_signer.get_address().await?,
//             factory_deps,
//             paymaster_params,
//         );

//         let signature = self
//             .sign_transaction(&execute_contract)
//             .await
//             .map_err(signing_failed_error)?;
//         Ok(signature)
//     }
// }
