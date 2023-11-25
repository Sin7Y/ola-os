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

pub fn load_mempool_config() -> Result<MempoolConfig, config::ConfigError> {
    load_config("configuration/mempool", "OLAOS_MEMPOOL")
}
