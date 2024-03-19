use serde::{Deserialize, Serialize};

pub use ola_circuits::stark::{
    config::StarkConfig,
    ola_stark::OlaStark,
    proof::{AllProof, BlockMetadata, PublicValues, TrieRoots},
    prover::prove_with_traces,
};
pub use olavm_plonky2::{
    field::{goldilocks_field::GoldilocksField, polynomial::PolynomialValues},
    plonk::config::{Blake3GoldilocksConfig, GenericConfig},
    util::timing::TimingTree,
};

pub const D: usize = 2;
pub const NUM_TABLES: usize = 12;
pub type C = Blake3GoldilocksConfig;
pub type F = <C as GenericConfig<D>>::F;

#[derive(Serialize, Deserialize, Clone)]
pub struct OlaBaseLayerCircuit {
    pub witness: [Vec<PolynomialValues<F>>; NUM_TABLES],
    pub public_values: PublicValues,
    pub config: StarkConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OlaBaseLayerProof {
    pub proof: AllProof<F, C, D>,
}
