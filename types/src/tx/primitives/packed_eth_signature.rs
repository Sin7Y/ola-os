use ethereum_types_old::H256 as ParityCryptoH256;
use ola_basic_types::{Address, H256};
use ola_utils::hash::hash_bytes;
use parity_crypto::publickey::{
    public_to_address, recover, sign, Error as ParityCryptoError, KeyPair,
    Signature as ETHSignature,
};

use super::{EIP712TypedStructure, Eip712Domain};
/// Struct used for working with ethereum signatures created using eth_sign (using geth, ethers.js, etc)
/// message is serialized as 65 bytes long `0x` prefixed string.
///
/// Some notes on implementation of methods of this structure:
///
/// Ethereum signed message produced by most clients contains v where v = 27 + recovery_id(0,1,2,3),
/// but for some clients v = recovery_id(0,1,2,3).
/// Library that we use for signature verification (written for bitcoin) expects v = recovery_id
///
/// That is why:
/// 1) when we create this structure by deserialization of message produced by user
/// we subtract 27 from v in `ETHSignature` if necessary and store it in the `ETHSignature` structure this way.
/// 2) When we serialize/create this structure we add 27 to v in `ETHSignature`.
///
/// This way when we have methods that consumes &self we can be sure that ETHSignature::recover_signer works
/// And we can be sure that we are compatible with Ethereum clients.
///
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackedEthSignature(ETHSignature);

impl PackedEthSignature {
    pub fn r(&self) -> &[u8] {
        self.0.r()
    }
    pub fn s(&self) -> &[u8] {
        self.0.s()
    }
    pub fn v(&self) -> u8 {
        self.0.v()
    }

    pub fn typed_data_to_signed_bytes(
        domain: &Eip712Domain,
        typed_struct: &impl EIP712TypedStructure,
    ) -> H256 {
        let mut bytes = Vec::new();
        bytes.extend_from_slice("\x19\x01".as_bytes());
        bytes.extend_from_slice(domain.hash_struct().as_bytes());
        bytes.extend_from_slice(typed_struct.hash_struct().as_bytes());
        hash_bytes(&bytes)
    }

    pub fn message_to_signed_bytes(msg: &[u8]) -> H256 {
        hash_bytes(msg)
    }

    pub fn sign_raw(
        private_key: &H256,
        signed_bytes: &H256,
    ) -> Result<PackedEthSignature, ParityCryptoError> {
        let private_key = ParityCryptoH256::from_slice(&private_key.0);
        let signed_bytes = ParityCryptoH256::from_slice(&signed_bytes.0);

        let secret_key = private_key.into();
        let signature = sign(&secret_key, &signed_bytes)?;
        Ok(PackedEthSignature(signature))
    }

    pub fn address_from_private_key(private_key: &H256) -> Result<Address, ParityCryptoError> {
        let private_key = ParityCryptoH256::from_slice(&private_key.0);
        let address = KeyPair::from_secret(private_key.into())?.address();
        Ok(Address::from(address.0))
    }
}
