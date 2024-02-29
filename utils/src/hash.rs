use ola_basic_types::H256;
use olavm_plonky2::hash::utils::poseidon_hash_bytes;

pub fn hash_bytes(inputs: &[u8]) -> H256 {
    let hash = poseidon_hash_bytes(inputs);
    H256::from(hash)
}

pub trait PoseidonBytes<T> {
    fn hash_bytes(&self) -> T
    where
        T: Sized;
}

impl<T> PoseidonBytes<[u8; 32]> for T
where
    T: AsRef<[u8]>,
{
    fn hash_bytes(&self) -> [u8; 32] {
        poseidon_hash_bytes(self.as_ref())
    }
}

pub trait Hasher {
    type Hash: AsRef<[u8]>;

    // Gets the hash of the byte sequence.
    fn hash_bytes(&self, value: &[u8]) -> Self::Hash;

    // Merges two hashes into one.
    fn compress(&self, lhs: &Self::Hash, rhs: &Self::Hash) -> Self::Hash;
}

#[derive(Default, Clone, Debug)]
pub struct PoseidonHasher;

impl Hasher for PoseidonHasher {
    type Hash = H256;

    fn hash_bytes(&self, value: &[u8]) -> H256 {
        hash_bytes(value)
    }

    fn compress(&self, lhs: &H256, rhs: &H256) -> H256 {
        let mut value = vec![];
        value.extend_from_slice(lhs.as_bytes());
        value.extend_from_slice(rhs.as_bytes());
        hash_bytes(&value)
    }
}
