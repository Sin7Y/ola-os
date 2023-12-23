use async_trait::async_trait;
use errors::SignerError;
use ola_types::request::TransactionRequest;
use ola_types::{tx::primitives::PackedEthSignature, Address};

pub mod abi;
pub mod errors;
pub mod key_store;
pub mod operation;
pub mod private_key_signer;
pub mod signer;
pub mod utils;
pub mod wallet;

pub trait OlaTxSigner: Send + Sync + Clone {
    fn sign_tx_request(&self, tx: TransactionRequest) -> Result<PackedEthSignature, SignerError>;
    fn sign_message(&self, message: &[u8]) -> Result<PackedEthSignature, SignerError>;
    fn get_address(&self) -> Result<Address, SignerError>;
}
