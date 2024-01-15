use std::time::Duration;

use serde::Deserialize;

use crate::load_config;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct FriProverGatewayConfig {
    pub api_url: String,
    pub api_poll_duration_secs: u16,
}

impl FriProverGatewayConfig {
    pub fn api_poll_duration(&self) -> Duration {
        Duration::from_secs(self.api_poll_duration_secs as u64)
    }
}

pub fn load_fri_prover_gateway_config() -> Result<FriProverGatewayConfig, config::ConfigError> {
    load_config("configuration/fri_prover_gateway", "OLAOS_FRI_PROVER_GATEWAY")
}

#[cfg(test)]
mod tests {
    use crate::{utils::tests::EnvMutex, fri_prover_gateway::load_fri_prover_gateway_config};

    use super::FriProverGatewayConfig;

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_fri_prover_gateway_config() -> FriProverGatewayConfig {
        FriProverGatewayConfig {
            api_url: "http://private-dns-for-server".to_string(),
            api_poll_duration_secs: 100,
        }
    }

    #[test]
    fn test_load_fri_prover_gateway_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_FRI_PROVER_GATEWAY_API_URL="http://private-dns-for-server"
            OLAOS_FRI_PROVER_GATEWAY_API_POLL_DURATION_SECS="100"
        "#;
        lock.set_env(config);

        let config = load_fri_prover_gateway_config().expect("failed to load fri prover gateway config");
        assert_eq!(config, default_fri_prover_gateway_config());
    }
}