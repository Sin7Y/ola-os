use std::{net::SocketAddr, time::Duration};

use serde::Deserialize;

use crate::{envy_load, load_config, BYTES_IN_MB};

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ApiConfig {
    pub web3_json_rpc: Web3JsonRpcConfig,
    pub healthcheck: HealthCheckConfig,
}

impl ApiConfig {
    pub fn from_env() -> Self {
        Self {
            web3_json_rpc: Web3JsonRpcConfig::from_env(),
            healthcheck: HealthCheckConfig::from_env(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Web3JsonRpcConfig {
    pub http_port: u16,
    pub http_url: String,
    pub ws_port: u16,
    pub ws_url: String,
    pub filters_limit: Option<u32>,
    pub threads_per_server: u32,
    pub max_nonce_ahead: u32,
    pub transactions_per_sec_limit: Option<u32>,
    pub max_tx_size: usize,
    pub vm_execution_cache_misses_limit: Option<usize>,
    pub vm_concurrency_limit: Option<usize>,
    pub http_threads: Option<u32>,
    pub ws_threads: Option<u32>,
    pub max_batch_request_size: Option<usize>,
    pub max_response_body_size_mb: Option<usize>,
    pub factory_deps_cache_size_mb: Option<usize>,
    pub initial_writes_cache_size_mb: Option<usize>,
    pub latest_values_cache_size_mb: Option<usize>,
    pub subscriptions_limit: Option<u32>,
    pub pubsub_polling_interval: Option<u64>,
}

impl Web3JsonRpcConfig {
    pub fn from_env() -> Self {
        envy_load("ola_web3_json_rpc", "OLAOS_WEB3_JSON_RPC_")
    }

    pub fn filters_limit(&self) -> usize {
        self.filters_limit.unwrap_or(10_000) as usize
    }

    pub fn subscriptions_limit(&self) -> usize {
        self.subscriptions_limit.unwrap_or(10000) as usize
    }

    pub fn pubsub_interval(&self) -> Duration {
        Duration::from_millis(self.pubsub_polling_interval.unwrap_or(200))
    }

    pub fn http_server_threads(&self) -> usize {
        self.http_threads.unwrap_or(self.threads_per_server) as usize
    }

    pub fn ws_server_threads(&self) -> usize {
        self.ws_threads.unwrap_or(self.threads_per_server) as usize
    }

    pub fn max_batch_request_size(&self) -> usize {
        self.max_batch_request_size.unwrap_or(500)
    }

    pub fn max_response_body_size(&self) -> usize {
        self.max_response_body_size_mb.unwrap_or(10) * BYTES_IN_MB
    }

    pub fn vm_concurrency_limit(&self) -> usize {
        self.vm_concurrency_limit.unwrap_or(2048)
    }

    pub fn factory_deps_cache_size(&self) -> usize {
        self.factory_deps_cache_size_mb.unwrap_or(128) * BYTES_IN_MB
    }

    pub fn initial_writes_cache_size(&self) -> usize {
        self.initial_writes_cache_size_mb.unwrap_or(32) * BYTES_IN_MB
    }

    pub fn latest_values_cache_size(&self) -> usize {
        self.latest_values_cache_size_mb.unwrap_or(128) * BYTES_IN_MB
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct HealthCheckConfig {
    /// Port to which the REST server is listening.
    pub port: u16,
}

impl HealthCheckConfig {
    pub fn from_env() -> Self {
        envy_load("healthcheck", "OLAOS_HEALTHCHECK_")
    }

    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }
}

pub fn load_api_config() -> Result<ApiConfig, config::ConfigError> {
    Ok(ApiConfig {
        web3_json_rpc: load_web3_json_rpc_config()?,
        healthcheck: load_healthcheck_config()?,
    })
}

pub fn load_web3_json_rpc_config() -> Result<Web3JsonRpcConfig, config::ConfigError> {
    load_config("configuration/web3_json_rpc", "OLAOS_WEB3_JSON_RPC")
}

pub fn load_healthcheck_config() -> Result<HealthCheckConfig, config::ConfigError> {
    load_config("configuration/health_check", "OLAOS_HEALTHCHECK")
}

#[cfg(test)]
mod tests {

    use super::{ApiConfig, HealthCheckConfig, Web3JsonRpcConfig};

    use crate::{api::load_api_config, utils::tests::EnvMutex};

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_config() -> ApiConfig {
        ApiConfig {
            web3_json_rpc: Web3JsonRpcConfig {
                http_port: 1001,
                http_url: "http://127.0.0.1:1001".to_string(),
                ws_port: 1002,
                ws_url: "ws://127.0.0.1:1002".to_string(),
                max_tx_size: 1_000_000,
                vm_execution_cache_misses_limit: None,
                vm_concurrency_limit: Some(2048),
                filters_limit: Some(10_000),
                threads_per_server: 128,
                http_threads: Some(128),
                ws_threads: Some(256),
                max_batch_request_size: Some(200),
                max_response_body_size_mb: Some(10),
                max_nonce_ahead: 5,
                transactions_per_sec_limit: Some(1000),
                factory_deps_cache_size_mb: Some(128),
                initial_writes_cache_size_mb: Some(32),
                latest_values_cache_size_mb: Some(128),
                subscriptions_limit: Some(10000),
                pubsub_polling_interval: Some(200),
            },
            healthcheck: HealthCheckConfig { port: 8081 },
        }
    }

    #[test]
    fn test_load_api_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_WEB3_JSON_RPC_HTTP_PORT="1001"
            OLAOS_WEB3_JSON_RPC_HTTP_URL="http://127.0.0.1:1001"
            OLAOS_WEB3_JSON_RPC_WS_PORT="1002"
            OLAOS_WEB3_JSON_RPC_WS_URL="ws://127.0.0.1:1002"
            OLAOS_WEB3_JSON_RPC_MAX_NONCE_AHEAD=5
            OLAOS_HEALTHCHECK_PORT=8081
        "#;
        lock.set_env(config);

        let api_config = load_api_config().expect("failed to load api config");
        assert_eq!(api_config, default_config());
    }
}
