use crate::{
    errors::{ClientError, KeystoreError, NumberConvertError, SignerError},
    utils::{concat_h256_u32_and_sha256, is_h256_a_valid_ola_hash},
};
use const_hex::encode;
use ethereum_types::{Public, Secret, H256, U256};
use ola_types::Address;
use ola_utils::{h256_to_u256, hash::PoseidonBytes, u256_to_h256};
use rand::{rngs::StdRng, Rng, SeedableRng};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
#[derive(Clone)]
pub struct OlaKeyPair {
    pub secret: Secret,
    pub public: Public,
    pub address: Address,
}

impl OlaKeyPair {
    pub fn from_random() -> Self {
        let mut rng = StdRng::from_entropy();
        let mut buffer = [0u8; 32];
        rng.fill(&mut buffer);

        let random_u256 = U256::from(&buffer);
        Self::from_etherum_private_key(u256_to_h256(random_u256)).unwrap()
    }

    pub fn new(secret: Secret) -> Result<Self, ClientError> {
        if !is_h256_a_valid_ola_hash(secret) {
            return Err(NumberConvertError::InvalidOlaHash(secret.to_string()))
                .map_err(ClientError::NumberConvertError)?;
        }
        let s = match SecretKey::from_slice(&secret[..]) {
            Ok(it) => it,
            Err(err) => {
                return Err(NumberConvertError::SecpError(err))
                    .map_err(ClientError::NumberConvertError)?
            }
        };

        let secp = Secp256k1::new();
        let pub_key = PublicKey::from_secret_key(&secp, &s);
        let serialized = pub_key.serialize_uncompressed();
        let mut public = Public::default();
        public.as_bytes_mut().copy_from_slice(&serialized[1..65]);
        let pub_x = H256::from_slice(&public[0..32]);
        let pub_y = H256::from_slice(&public[32..64]);
        if !is_h256_a_valid_ola_hash(pub_x) {
            return Err(NumberConvertError::InvalidOlaHash(pub_x.to_string()))
                .map_err(ClientError::NumberConvertError)?;
        }
        if !is_h256_a_valid_ola_hash(pub_y) {
            return Err(NumberConvertError::InvalidOlaHash(pub_y.to_string()))
                .map_err(ClientError::NumberConvertError)?;
        }
        let address = H256::from_slice(&public.hash_bytes());
        if !is_h256_a_valid_ola_hash(address.clone()) {
            return Err(NumberConvertError::InvalidOlaHash(address.to_string()))
                .map_err(ClientError::NumberConvertError)?;
        }
        Ok(OlaKeyPair {
            secret,
            public,
            address,
        })
    }

    pub fn from_etherum_private_key(private_key: Secret) -> Result<Self, ClientError> {
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
                        return Err(SignerError::InvalidPrivateKey(private_key))
                            .map_err(ClientError::SigningError)?;
                    }
                }
            }
        }
    }

    pub fn from_keystore<P>(path: P, password: &str) -> Result<Self, ClientError>
    where
        P: AsRef<std::path::Path>,
    {
        let key_vec = eth_keystore::decrypt_key(path, password)
            .map_err(KeystoreError::Inner)
            .map_err(ClientError::KeystoreError)?;
        let mut key = [0u8; 32];
        if key_vec.len() == 32 {
            key.copy_from_slice(&key_vec);
        } else {
            return Err(KeystoreError::InvalidScalar).map_err(ClientError::KeystoreError)?;
        }
        Self::new(H256(key))
    }

    pub fn save_as_keystore<P>(&self, path: P, password: &str) -> Result<(), KeystoreError>
    where
        P: AsRef<std::path::Path>,
    {
        let mut path = path.as_ref().to_path_buf();
        let file_name = path
            .file_name()
            .ok_or(KeystoreError::InvalidPath)?
            .to_str()
            .ok_or(KeystoreError::InvalidPath)?
            .to_owned();
        path.pop();

        let mut rng = StdRng::from_entropy();
        eth_keystore::encrypt_key(path, &mut rng, self.secret, password, Some(&file_name))
            .map_err(KeystoreError::Inner)?;

        Ok(())
    }

    pub fn private_key_str(&self) -> String {
        encode(&self.secret)
    }

    pub fn public_key_str(&self) -> String {
        encode(&self.public)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethereum_types::H256;
    use ethereum_types::H512;
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
            H256::from_str("0x2991c0899fee28da35e005cb4947131b27b9274008810b30adb209e8525bddeb")
                .unwrap()
        );
    }
}
