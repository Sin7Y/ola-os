use crate::{envy_load, load_config};
use ola_basic_types::{network::Network, Address, H256};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq, Default)]
pub struct SequencerConfig {
    pub miniblock_seal_queue_capacity: usize,
    pub miniblock_commit_deadline_ms: u64,
    pub block_commit_deadline_ms: u64,
    pub reject_tx_at_geometry_percentage: f64,
    pub close_block_at_geometry_percentage: f64,
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
    pub ola_network_id: u16,
    pub ola_network_name: String,
}

impl NetworkConfig {
    pub fn from_env() -> Self {
        envy_load("ola_network", "OLAOS_NETWORK_")
    }
}

pub fn load_sequencer_config() -> Result<SequencerConfig, config::ConfigError> {
    load_config("configuration/sequencer", "OLAOS_SEQUENCER")
}

pub fn load_network_config() -> Result<NetworkConfig, config::ConfigError> {
    load_config("configuration/network", "OLAOS_NETWORK")
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ola_basic_types::{network::Network, Address, H256};

    use crate::{
        sequencer::{load_network_config, load_sequencer_config},
        utils::tests::EnvMutex,
    };

    use super::{NetworkConfig, SequencerConfig};

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_sequencer_config() -> SequencerConfig {
        SequencerConfig {
            miniblock_seal_queue_capacity: 10,
            miniblock_commit_deadline_ms: 1000,
            block_commit_deadline_ms: 2500,
            reject_tx_at_geometry_percentage: 0.3,
            close_block_at_geometry_percentage: 0.5,
            fee_account_addr: Address::from_str("0xde03a0B5963f75f1C8485B355fF6D30f3093BDE7C8485B355fF6D30f3093BDE7")
                .unwrap(),
            entrypoint_hash: H256::from_str(
                "0x0100038581be3d0e201b3cc45d151ef5cc59eb3a0f146ad44f0f72abf00b594c",
            )
            .unwrap(),
            default_aa_hash: H256::from_str(
                "0x0100038dc66b69be75ec31653c64cb931678299b9b659472772b2550b703f41c",
            )
            .unwrap(),
            transaction_slots: 250,
            save_call_traces: true,
        }
    }

    fn default_network_config() -> NetworkConfig {
        NetworkConfig {
            network: Network::Localhost,
            ola_network_id: 360,
            ola_network_name: "localhost".to_string(),
        }
    }
    #[test]
    fn test_load_sequencer_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_SEQUENCER_FEE_ACCOUNT_ADDR=0xde03a0B5963f75f1C8485B355fF6D30f3093BDE7C8485B355fF6D30f3093BDE7
            OLAOS_SEQUENCER_ENTRYPOINT_HASH=0x0100038581be3d0e201b3cc45d151ef5cc59eb3a0f146ad44f0f72abf00b594c
            OLAOS_SEQUENCER_DEFAULT_AA_HASH=0x0100038dc66b69be75ec31653c64cb931678299b9b659472772b2550b703f41c
            OLAOS_SEQUENCER_CLOSE_BLOCK_AT_GEOMETRY_PERCENTAGE=0.5
        "#;
        lock.set_env(config);

        let sequencer_config = load_sequencer_config().expect("failed to load sequencer config");
        assert_eq!(sequencer_config, default_sequencer_config());
    }

    #[test]
    fn test_load_network_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_NETWORK_OLA_NETWORK_ID=360
        "#;
        lock.set_env(config);

        let network_config = load_network_config().expect("failed to load db config");
        assert_eq!(network_config, default_network_config());
    }
}
