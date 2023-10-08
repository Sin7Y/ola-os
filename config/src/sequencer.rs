use crate::envy_load;
use ola_basic_types::{network::Network, Address, H256};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq, Default)]
pub struct SequencerConfig {
    pub fee_account_addr: Address,
    pub entrypoint_hash: H256,
    pub default_aa_hash: H256,
}

impl SequencerConfig {
    pub fn from_env() -> Self {
        envy_load("ola_sequencer", "OLA_SEQUENCER_")
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct NetworkConfig {
    pub network: Network,
    pub ola_network_id: u16,
    pub ola_network_name: String,
}

impl NetworkConfig {
    pub fn from_env() -> Self {
        envy_load("ola_network", "OLA_CHAIN_ETH_")
    }
}
