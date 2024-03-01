use std::net::SocketAddr;

use serde::Deserialize;

use crate::load_config;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct OffChainVerifierConfig {
    pub port: u16,
}

impl OffChainVerifierConfig {
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }
}

pub fn load_offchain_verifier_config() -> Result<OffChainVerifierConfig, config::ConfigError> {
    load_config("configuration/offchain_verifier", "OLAOS_OFFCHAIN_VERIFIER")
}

#[cfg(test)]
mod tests {
    use crate::utils::tests::EnvMutex;

    use super::{load_offchain_verifier_config, OffChainVerifierConfig};

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_offchain_verifier_config() -> OffChainVerifierConfig {
        OffChainVerifierConfig { port: 13003 }
    }

    #[test]
    fn test_load_offchain_verifier_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_OFFCHAIN_VERIFIER_PORT="13003"
        "#;
        lock.set_env(config);

        let config =
            load_offchain_verifier_config().expect("failed to load offchain verifier config");
        assert_eq!(config, default_offchain_verifier_config());
    }
}
