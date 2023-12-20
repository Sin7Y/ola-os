use async_trait::async_trait;
use errors::SignerError;
use ola_types::tx::primitives::EIP712TypedStructure;
use ola_types::tx::primitives::Eip712Domain;
use ola_types::{tx::primitives::PackedEthSignature, Address};

pub mod abi;
pub mod errors;
pub mod key_store;
pub mod operation;
pub mod private_key_signer;
pub mod signer;
pub mod utils;
pub mod wallet;
#[async_trait]
pub trait EthereumSigner: Send + Sync + Clone {
    async fn sign_message(&self, message: &[u8]) -> Result<PackedEthSignature, SignerError>;
    async fn sign_typed_data<S: EIP712TypedStructure + Sync>(
        &self,
        domain: &Eip712Domain,
        typed_struct: &S,
    ) -> Result<PackedEthSignature, SignerError>;
    async fn get_address(&self) -> Result<Address, SignerError>;
}
