use errors::SignerError;
use ola_types::{
    tx::primitives::{EIP712TypedStructure, Eip712Domain, PackedEthSignature},
    Address,
};

pub mod errors;
pub mod key_store;
pub mod utils;

// pub trait EthereumSigner: Send + Sync + Clone {
//     async fn sign_message(&self, message: &[u8]) -> Result<PackedEthSignature, SignerError>;
//     async fn sign_typed_data<S: EIP712TypedStructure + Sync>(
//         &self,
//         domain: &Eip712Domain,
//         typed_struct: &S,
//     ) -> Result<PackedEthSignature, SignerError>;
//     async fn sign_transaction(&self, raw_tx: TransactionParameters)
//         -> Result<Vec<u8>, SignerError>;
//     async fn get_address(&self) -> Result<Address, SignerError>;
// }
