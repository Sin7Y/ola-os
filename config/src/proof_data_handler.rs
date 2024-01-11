use std::time::Duration;

use serde::Deserialize;

use crate::load_config;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
pub enum ProtocolVersionLoadingMode {
    FromDb,
    FromEnvVar,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ProofDataHandlerConfig {
    pub http_port: u16,
    pub proof_generation_timeout_in_secs: u16,
    pub protocol_version_loading_mode: ProtocolVersionLoadingMode,
    pub fri_protocol_version_id: u16,
}
impl ProofDataHandlerConfig {
    pub fn proof_generation_timeout(&self) -> Duration {
        Duration::from_secs(self.proof_generation_timeout_in_secs as u64)
    }
}

pub fn load_proof_data_handler_config() -> Result<ProofDataHandlerConfig, config::ConfigError> {
    load_config(
        "configuration/proof_data_handler",
        "OLAOS_PROOF_DATA_HANDLER",
    )
}

#[cfg(test)]
mod tests {
    use crate::utils::tests::EnvMutex;

    use super::{
        load_proof_data_handler_config, ProofDataHandlerConfig, ProtocolVersionLoadingMode,
    };

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_proof_data_handler_config() -> ProofDataHandlerConfig {
        ProofDataHandlerConfig {
            http_port: 13320,
            proof_generation_timeout_in_secs: 18000,
            protocol_version_loading_mode: ProtocolVersionLoadingMode::FromEnvVar,
            fri_protocol_version_id: 2,
        }
    }

    #[test]
    fn test_load_object_store_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_PROOF_DATA_HANDLER_PROOF_GENERATION_TIMEOUT_IN_SECS="18000"
            OLAOS_PROOF_DATA_HANDLER_HTTP_PORT="13320"
            OLAOS_PROOF_DATA_HANDLER_PROTOCOL_VERSION_LOADING_MODE="FromEnvVar"
            OLAOS_PROOF_DATA_HANDLER_FRI_PROTOCOL_VERSION_ID="2"
        "#;
        lock.set_env(config);

        let config =
            load_proof_data_handler_config().expect("failed to load proof data handler config");
        assert_eq!(config, default_proof_data_handler_config());
    }
}
