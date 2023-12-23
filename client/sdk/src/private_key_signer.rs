use ola_types::{request::TransactionRequest, tx::primitives::PackedEthSignature, Address};

use crate::{errors::SignerError, key_store::OlaKeyPair, OlaTxSigner};

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

impl OlaTxSigner for PrivateKeySigner {
    fn sign_tx_request(&self, tx: TransactionRequest) -> Result<PackedEthSignature, SignerError> {
        let message = tx.into_signed_bytes();
        let signature = PackedEthSignature::sign(&self.key_pair.secret, message.as_slice())
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(signature)
    }

    fn sign_message(&self, message: &[u8]) -> Result<PackedEthSignature, SignerError> {
        let signature = PackedEthSignature::sign(&self.key_pair.secret, message)
            .map_err(|err| SignerError::SigningFailed(err.to_string()))?;
        Ok(signature)
    }

    fn get_address(&self) -> Result<Address, SignerError> {
        Ok(self.key_pair.address)
    }
}
