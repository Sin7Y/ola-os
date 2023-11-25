use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{envy_load, load_config};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MerkleTreeNode {
    #[default]
    Full,
    Lightweight,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MerkleTreeConfig {
    #[serde(default = "MerkleTreeConfig::default_path")]
    pub path: String,
    #[serde(default = "MerkleTreeConfig::default_backup_path")]
    pub backup_path: String,
    #[serde(default)]
    pub mode: MerkleTreeNode,
    #[serde(default = "MerkleTreeConfig::default_multi_get_chunk_size")]
    pub multi_get_chunk_size: usize,
    #[serde(default = "MerkleTreeConfig::default_block_cache_size_mb")]
    pub block_cache_size_mb: usize,
    #[serde(default = "MerkleTreeConfig::default_max_l1_batches_per_iter")]
    pub max_l1_batches_per_iter: usize,
}

impl Default for MerkleTreeConfig {
    fn default() -> Self {
        Self {
            path: Self::default_path(),
            backup_path: Self::default_backup_path(),
            mode: MerkleTreeNode::default(),
            multi_get_chunk_size: Self::default_multi_get_chunk_size(),
            block_cache_size_mb: Self::default_block_cache_size_mb(),
            max_l1_batches_per_iter: Self::default_max_l1_batches_per_iter(),
        }
    }
}

impl MerkleTreeConfig {
    fn default_path() -> String {
        "./db/lightweight-new".to_owned()
    }

    fn default_backup_path() -> String {
        "./db/backups".to_owned()
    }

    const fn default_multi_get_chunk_size() -> usize {
        500
    }

    const fn default_block_cache_size_mb() -> usize {
        128
    }

    const fn default_max_l1_batches_per_iter() -> usize {
        20
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DBConfig {
    pub statement_timeout_sec: Option<u64>,
    #[serde(default = "DBConfig::default_sequencer_db_path")]
    pub sequencer_db_path: String,
    #[serde(skip)]
    pub merkle_tree: MerkleTreeConfig,
    #[serde(default = "DBConfig::default_backup_count")]
    pub backup_count: usize,
    #[serde(default = "DBConfig::default_backup_interval_ms")]
    pub backup_interval_ms: u64,
}

impl DBConfig {
    fn default_sequencer_db_path() -> String {
        "./db/sequencer".to_owned()
    }

    const fn default_backup_count() -> usize {
        5
    }

    const fn default_backup_interval_ms() -> u64 {
        60_000
    }

    pub fn from_env() -> Self {
        Self {
            merkle_tree: envy_load("ola_database_merkle_tree", "OLAOS_MERKLE_TREE_"),
            ..envy_load("ola_database", "OLAOS_DATABASE_")
        }
    }

    pub fn statement_timeout(&self) -> Option<Duration> {
        self.statement_timeout_sec.map(Duration::from_secs)
    }

    pub fn backup_interval(&self) -> Duration {
        Duration::from_millis(self.backup_interval_ms)
    }
}

pub fn load_db_config() -> Result<DBConfig, config::ConfigError> {
    Ok(DBConfig {
        merkle_tree: load_merkle_tree_config()?,
        ..load_config("configuration/database", "OLAOS_DATABASE")?
    })
    // load_config("configuration/database", "OLAOS_DATABASE")
}

pub fn load_merkle_tree_config() -> Result<MerkleTreeConfig, config::ConfigError> {
    load_config("configuration/merkle_tree", "OLAOS_MERKLE_TREE")
}

#[cfg(test)]
mod tests {
    use crate::utils::tests::EnvMutex;

    use super::{load_db_config, DBConfig, MerkleTreeConfig};

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_db_config() -> DBConfig {
        DBConfig {
            statement_timeout_sec: Some(300),
            sequencer_db_path: "./db/main/sequencer".to_string(),
            merkle_tree: MerkleTreeConfig {
                path: "./db/main/tree".to_string(),
                backup_path: "./db/main/backups".to_string(),
                mode: Default::default(),
                multi_get_chunk_size: 1000,
                block_cache_size_mb: 128,
                max_l1_batches_per_iter: 50,
            },
            backup_count: 5,
            backup_interval_ms: 60000,
        }
    }

    #[test]
    fn test_load_database_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_DATABASE_STATEMENT_TIMEOUT_SEC=300
            OLAOS_DATABASE_SEQUENCER_DB_PATH=./db/main/sequencer
            OLAOS_DATABASE_BACKUP_COUNT=5
            OLAOS_DATABASE_BACKUP_INTERVAL_MS=60000
        "#;
        lock.set_env(config);

        let db_config = load_db_config().expect("failed to load db config");
        assert_eq!(db_config, default_db_config());
    }
}
