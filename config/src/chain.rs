use std::time::Duration;

use serde::Deserialize;

use crate::{envy_load, load_config};

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct MempoolConfig {
    pub sync_interval_ms: u64,
    pub sync_batch_size: usize,
    pub capacity: u64,
    pub stuck_tx_timeout: u64,
    pub remove_stuck_txs: bool,
    pub delay_interval: u64,
}

impl MempoolConfig {
    pub fn sync_interval(&self) -> Duration {
        Duration::from_millis(self.sync_interval_ms)
    }

    pub fn stuck_tx_timeout(&self) -> Duration {
        Duration::from_secs(self.stuck_tx_timeout)
    }

    pub fn delay_interval(&self) -> Duration {
        Duration::from_millis(self.delay_interval)
    }

    pub fn from_env() -> Self {
        envy_load("mempool", "OLAOS_MEMPOOL_")
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct OperationsManagerConfig {
    /// Sleep time in ms when there is no new input data
    pub delay_interval: u64,
}

impl OperationsManagerConfig {
    pub fn from_env() -> Self {
        envy_load("operations_manager", "OLAOS_OPERATIONS_MANAGER_")
    }

    pub fn delay_interval(&self) -> Duration {
        Duration::from_millis(self.delay_interval)
    }
}

pub fn load_mempool_config() -> Result<MempoolConfig, config::ConfigError> {
    load_config("configuration/mempool", "OLAOS_MEMPOOL")
}

pub fn load_operation_manager_config() -> Result<OperationsManagerConfig, config::ConfigError> {
    load_config(
        "configuration/operation_manager",
        "OLAOS_OPERATIONS_MANAGER",
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        chain::{load_mempool_config, load_operation_manager_config},
        utils::tests::EnvMutex,
    };

    use super::{MempoolConfig, OperationsManagerConfig};

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_mempool_config() -> MempoolConfig {
        MempoolConfig {
            sync_interval_ms: 10,
            sync_batch_size: 1000,
            capacity: 10000,
            stuck_tx_timeout: 50,
            remove_stuck_txs: true,
            delay_interval: 200,
        }
    }

    fn default_operation_manager_config() -> OperationsManagerConfig {
        OperationsManagerConfig {
            delay_interval: 100,
        }
    }

    #[test]
    fn test_load_mempool_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_MEMPOOL_CAPACITY=10000
            OLAOS_MEMPOOL_STUCK_TX_TIMEOUT=50
            OLAOS_MEMPOOL_DELAY_INTERVAL=200
        "#;
        lock.set_env(config);

        let api_config = load_mempool_config().expect("failed to load mempool config");
        assert_eq!(api_config, default_mempool_config());
    }

    #[test]
    fn test_load_operation_manager_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_OPERATIONS_MANAGER_DELAY_INTERVAL=100
        "#;
        lock.set_env(config);

        let api_config =
            load_operation_manager_config().expect("failed to load operation manager config");
        assert_eq!(api_config, default_operation_manager_config());
    }
}
