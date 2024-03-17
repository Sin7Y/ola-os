use std::time::Duration;

use serde::Deserialize;

use crate::load_config;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct FriProverConfig {
    // pub setup_data_path: String,
    pub max_attempts: u32,
    pub generation_timeout_in_secs: u16,
    // pub base_layer_circuit_ids_to_be_verified: Vec<u8>,
    // pub recursive_layer_circuit_ids_to_be_verified: Vec<u8>,
    // pub setup_load_mode: SetupLoadMode,
    // pub specialized_group_id: u8,
    // pub witness_vector_generator_thread_count: Option<usize>,
    // pub queue_capacity: usize,
    // pub zone_read_url: String,

    // whether to write to public GCS bucket for https://github.com/matter-labs/era-boojum-validator-cli
    pub shall_save_to_public_bucket: bool,
}

impl FriProverConfig {
    pub fn proof_generation_timeout(&self) -> Duration {
        Duration::from_secs(self.generation_timeout_in_secs as u64)
    }
}

pub fn load_prover_fri_config() -> Result<FriProverConfig, config::ConfigError> {
    load_config("configuration/fri_prover", "OLAOS_FRI_PROVER")
}

#[cfg(test)]
mod tests {
    use crate::{fri_prover::load_prover_fri_config, utils::tests::EnvMutex};

    use super::FriProverConfig;

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_fri_prover_config() -> FriProverConfig {
        FriProverConfig {
            max_attempts: 10,
            generation_timeout_in_secs: 300,
            shall_save_to_public_bucket: true,
        }
    }

    #[test]
    fn test_load_fri_prover_gateway_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
        OLAOS_FRI_PROVER_GENERATION_TIMEOUT_IN_SECS=300
        OLAOS_FRI_PROVER_MAX_ATTEMPTS=10
        OLAOS_FRI_PROVER_SHALL_SAVE_TO_PUBLIC_BUCKET=true
        "#;
        lock.set_env(config);

        let config = load_prover_fri_config().expect("failed to load fri prover config");
        assert_eq!(config, default_fri_prover_config());
    }
}
