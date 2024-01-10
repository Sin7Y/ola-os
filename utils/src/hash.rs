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
