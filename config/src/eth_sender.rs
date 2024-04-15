use ola_basic_types::H256;
use serde::Deserialize;

use crate::load_config;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ETHSenderConfig {
    pub sender: SenderConfig,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct SenderConfig {
    pub wait_confirmations: Option<u64>,
}

impl SenderConfig {
    // Don't load private key, if it's not required.
    pub fn private_key(&self) -> Option<H256> {
        std::env::var("OLAOS_ETH_SENDER_OPERATOR_PRIVATE_KEY")
            .or_else(|e| {
                Ok("1bcb518fd7c0176670f800a107ea75bb6ff31e83edc29700cbfcff40b06a0292".to_string())
            })
            .ok()
            .map(|pk| {
                pk.parse()
                    .expect("failed to load OLAOS_ETH_SENDER_OPERATOR_PRIVATE_KEY")
            })
    }
}

pub fn load_eth_sender_config() -> Result<ETHSenderConfig, config::ConfigError> {
    let sender_config = load_sender_config().expect("failed to load sender config");
    Ok(ETHSenderConfig {
        sender: sender_config,
    })
}

pub fn load_sender_config() -> Result<SenderConfig, config::ConfigError> {
    load_config("configuration/eth_sender", "OLAOS_ETH_SENDER")
}

#[cfg(test)]
mod tests {
    use crate::{eth_sender::load_sender_config, utils::tests::EnvMutex};

    use super::SenderConfig;

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_sender_config() -> SenderConfig {
        SenderConfig {
            wait_confirmations: Some(10),
        }
    }

    #[test]
    fn test_load_sender_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_ETH_SENDER_WAIT_CONFIRMATIONS=10
        "#;
        lock.set_env(config);

        let sender_config = load_sender_config().expect("failed to load sender config");
        assert_eq!(sender_config, default_sender_config());
    }
}
