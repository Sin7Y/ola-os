use crate::{envy_load, load_config};
use ola_basic_types::{network::Network, Address, H256};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq, Default)]
pub struct SequencerConfig {
    pub miniblock_seal_queue_capacity: usize,
    pub miniblock_commit_deadline_ms: u64,
    pub block_commit_deadline_ms: u64,
    pub fee_account_addr: Address,
    pub entrypoint_hash: H256,
    pub default_aa_hash: H256,
    pub transaction_slots: usize,
    pub save_call_traces: bool,
}

impl SequencerConfig {
    pub fn from_env() -> Self {
        envy_load("ola_sequencer", "OLAOS_SEQUENCER_")
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct NetworkConfig {
    pub network: Network,
    pub ola_network_id: u64,
    pub ola_network_name: String,
}

impl NetworkConfig {
    pub fn from_env() -> Self {
        envy_load("ola_network", "OLAOS_CHAIN_ETH_")
    }
}

pub fn load_sequencer_config() -> Result<SequencerConfig, config::ConfigError> {
    load_config("config/configuration/sequencer")
}

pub fn load_network_config() -> Result<NetworkConfig, config::ConfigError> {
    load_config("config/configuration/network")
}
