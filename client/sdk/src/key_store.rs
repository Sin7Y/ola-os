use crate::{
    errors::{NumberConvertError, SignerError},
    utils::{concat_h256_u32_and_sha256, is_h256_a_valid_ola_hash},
};
use ethereum_types::{Public, Secret, H256};
use ola_types::Address;
use parity_crypto::Keccak256;
use secp256k1::{PublicKey, Secp256k1, SecretKey};
#[derive(Clone)]
pub struct OlaKeyPair {
    pub secret: Secret,
    pub public: Public,
    pub address: Address,
}

impl OlaKeyPair {
    fn new(secret: Secret) -> Result<Self, NumberConvertError> {
        if !is_h256_a_valid_ola_hash(secret) {
            return Err(NumberConvertError::InvalidOlaHash(secret.to_string()));
        }
        let s = match SecretKey::from_slice(&secret[..]) {
            Ok(it) => it,
            Err(err) => return Err(NumberConvertError::SecpError(err)),
        };

        let secp = Secp256k1::new();
        let pub_key = PublicKey::from_secret_key(&secp, &s);
        let serialized = pub_key.serialize_uncompressed();
        let mut public = Public::default();
        public.as_bytes_mut().copy_from_slice(&serialized[1..65]);
        let pub_x = H256::from_slice(&public[0..32]);
        let pub_y = H256::from_slice(&public[32..64]);
        if !is_h256_a_valid_ola_hash(pub_x) {
            return Err(NumberConvertError::InvalidOlaHash(pub_x.to_string()));
        }
        if !is_h256_a_valid_ola_hash(pub_y) {
            return Err(NumberConvertError::InvalidOlaHash(pub_y.to_string()));
        }

        let address = H256::from_slice(&public.keccak256());
        if !is_h256_a_valid_ola_hash(address.clone()) {
            return Err(NumberConvertError::InvalidOlaHash(address.to_string()));
        }
        Ok(OlaKeyPair {
            secret,
            public,
            address,
        })
    }

    fn from_etherum_private_key(private_key: Secret) -> Result<Self, SignerError> {
        let mut i: u32 = 0;
        loop {
            let secret = concat_h256_u32_and_sha256(private_key, i);
            let key_pair = OlaKeyPair::new(secret);
            match key_pair {
                Ok(it) => return Ok(it),
                Err(_) => {
                    if i < 10000 {
                        i += 1
                    } else {
                        return Err(SignerError::InvalidPrivateKey(private_key));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethereum_types::H512;
    use ethereum_types::H256;
    use std::str::FromStr;

    #[test]
    fn test_new() {
        let private_key =
            H256::from_str("a100df7a048e50ed308ea696dc600215098141cb391e9527329df289f9383f65")
                .unwrap();
        let key_pair = OlaKeyPair::new(private_key).unwrap();
        assert_eq!(
            key_pair.secret,
            H256::from_str("a100df7a048e50ed308ea696dc600215098141cb391e9527329df289f9383f65")
                .unwrap()
        );
        assert_eq!(
            key_pair.public,
            H512::from_str("8ce0db0b0359ffc5866ba61903cc2518c3675ef2cf380a7e54bde7ea20e6fa1ab45b7617346cd11b7610001ee6ae5b0155c41cad9527cbcdff44ec67848943a4")
                .unwrap()
        );
        assert_eq!(
            key_pair.address,
            H256::from_str("0xb665f9be1919998d337476305b073e9233944b5e729e46d618f0d8edf3d9c34a")
                .unwrap()
        );
    }
}
