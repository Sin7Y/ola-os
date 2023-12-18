use ethereum_types::Address;
use ola_types::tx::primitives::{EIP712TypedStructure, Eip712Domain, PackedEthSignature};

use crate::{errors::SignerError, key_store::OlaKeyPair, EthereumSigner};

#[derive(Clone)]
pub struct PrivateKeySigner {
    key_pair: OlaKeyPair,
}

impl std::fmt::Debug for PrivateKeySigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PrivateKeySigner")
    }
}

impl PrivateKeySigner {
    pub fn new(key_pair: OlaKeyPair) -> Self {
        Self { key_pair }
    }
}

#[async_trait::async_trait]
impl EthereumSigner for PrivateKeySigner {
    async fn sign_message(&self, message: &[u8]) -> Result<PackedEthSignature, SignerError> {
        let signature = PackedEthSignature::sign(&self.key_pair.secret, message)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(signature)
    }

    async fn sign_typed_data<S: EIP712TypedStructure + Sync>(
        &self,
        domain: &Eip712Domain,
        typed_struct: &S,
    ) -> Result<PackedEthSignature, SignerError> {
        let signature =
            PackedEthSignature::sign_typed_data(&self.key_pair.secret, domain, typed_struct)
                .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(signature)
    }

    async fn get_address(&self) -> Result<Address, SignerError> {
        Ok(self.key_pair.address)
    }
}
