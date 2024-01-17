use std::{string, time::Duration};

// Built-in uses
// External uses
use serde::{Deserialize, Deserializer};

use crate::load_config;

/// Configuration for the fri witness generation
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct FriWitnessGeneratorConfig {
    /// Max time for witness to be generated
    pub generation_timeout_in_secs: u16,
    /// Max attempts for generating witness
    pub max_attempts: u32,
    // Percentage of the blocks that gets proven in the range [0.0, 1.0]
    // when 0.0 implies all blocks are skipped and 1.0 implies all blocks are proven.
    pub blocks_proving_percentage: Option<u8>,
    #[serde(deserialize_with = "string_to_vec_u32")]
    pub dump_arguments_for_blocks: Vec<u32>,
    // Optional l1 batch number to process block until(inclusive).
    // This parameter is used in case of performing circuit upgrades(VK/Setup keys),
    // to not let witness-generator pick new job and finish all the existing jobs with old circuit.
    #[serde(default)]
    pub last_l1_batch_to_process: Option<u32>,
    // Force process block with specified number when sampling is enabled.
    pub force_process_block: Option<u32>,

    // whether to write to public GCS bucket for https://github.com/matter-labs/era-boojum-validator-cli
    pub shall_save_to_public_bucket: bool,
}
impl FriWitnessGeneratorConfig {
    pub fn witness_generation_timeout(&self) -> Duration {
        Duration::from_secs(self.generation_timeout_in_secs as u64)
    }

    pub fn last_l1_batch_to_process(&self) -> u32 {
        self.last_l1_batch_to_process.unwrap_or(u32::MAX)
    }
}

pub fn load_fri_witness_generator_config() -> Result<FriWitnessGeneratorConfig, config::ConfigError>
{
    load_config("configuration/fri_witness_generator", "OLAOS_FRI_WITNESS")
}

pub fn string_to_vec_u32<'de, D>(deserializer: D) -> Result<Vec<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;

    let res = string
        .split(',')
        .map(|s| s.parse::<u32>())
        .filter_map(Result::ok)
        .collect();
    Ok(res)
}

#[cfg(test)]
mod tests {
    use crate::{fri_witness_generator::load_fri_witness_generator_config, utils::tests::EnvMutex};

    use super::FriWitnessGeneratorConfig;

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_fri_witness_generator_config() -> FriWitnessGeneratorConfig {
        FriWitnessGeneratorConfig {
            generation_timeout_in_secs: 900u16,
            max_attempts: 4,
            blocks_proving_percentage: Some(30),
            dump_arguments_for_blocks: vec![2, 3],
            last_l1_batch_to_process: None,
            force_process_block: Some(1),
            shall_save_to_public_bucket: true,
        }
    }

    #[test]
    fn test_load_fri_prover_gateway_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
        OLAOS_FRI_WITNESS_GENERATION_TIMEOUT_IN_SECS=900
        OLAOS_FRI_WITNESS_MAX_ATTEMPTS=4
        OLAOS_FRI_WITNESS_DUMP_ARGUMENTS_FOR_BLOCKS="2,3"
        OLAOS_FRI_WITNESS_BLOCKS_PROVING_PERCENTAGE="30"
        OLAOS_FRI_WITNESS_FORCE_PROCESS_BLOCK="1"
        OLAOS_FRI_WITNESS_SHALL_SAVE_TO_PUBLIC_BUCKET=true
        "#;
        lock.set_env(config);

        let config =
            load_fri_witness_generator_config().expect("failed to load fri prover gateway config");
        assert_eq!(config, default_fri_witness_generator_config());
    }
}
