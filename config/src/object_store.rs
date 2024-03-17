use serde::Deserialize;

use crate::load_config;

#[derive(Debug, Deserialize, Eq, PartialEq, Clone, Copy)]
pub enum ObjectStoreMode {
    FileBacked,
}

/// Configuration for the object store
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ObjectStoreConfig {
    pub bucket_base_url: String,
    pub mode: ObjectStoreMode,
    pub file_backed_base_path: String,
    pub gcs_credential_file_path: String,
    pub max_retries: u16,
}

#[derive(Debug)]
pub struct PublicObjectStoreConfig(pub ObjectStoreConfig);

#[derive(Debug)]
pub struct ProverObjectStoreConfig(pub ObjectStoreConfig);

pub fn load_object_store_config() -> Result<ObjectStoreConfig, config::ConfigError> {
    load_config("configuration/object_store", "OLAOS_OBJECT_STORE")
}

pub fn load_public_object_store_config() -> Result<PublicObjectStoreConfig, config::ConfigError> {
    let config = load_config(
        "configuration/public_object_store",
        "OLAOS_PUBLIC_OBJECT_STORE",
    )
    .expect("failed to load public object store config");
    Ok(PublicObjectStoreConfig(config))
}

pub fn load_prover_object_store_config() -> Result<ProverObjectStoreConfig, config::ConfigError> {
    let config = load_config(
        "configuration/prover_object_store",
        "OLAOS_PROVER_OBJECT_STORE",
    )
    .expect("failed to load prover object store config");
    Ok(ProverObjectStoreConfig(config))
}

#[cfg(test)]
mod tests {
    use crate::utils::tests::EnvMutex;

    use super::{load_object_store_config, ObjectStoreConfig, ObjectStoreMode};

    static MUTEX: EnvMutex = EnvMutex::new();

    fn default_object_store_config() -> ObjectStoreConfig {
        ObjectStoreConfig {
            bucket_base_url: "public_base_url".to_string(),
            mode: ObjectStoreMode::FileBacked,
            file_backed_base_path: "artifacts".to_string(),
            gcs_credential_file_path: "/path/to/gcs_credentials.json".to_string(),
            max_retries: 5,
        }
    }

    #[test]
    fn test_load_object_store_config() {
        let mut lock = MUTEX.lock();
        let config = r#"
            OLAOS_OBJECT_STORE_BUCKET_BASE_URL="public_base_url"
            OLAOS_OBJECT_STORE_MODE="FileBacked"
            OLAOS_OBJECT_STORE_FILE_BACKED_BASE_PATH="artifacts"
            OLAOS_OBJECT_STORE_GCS_CREDENTIAL_FILE_PATH="/path/to/gcs_credentials.json"
            OLAOS_OBJECT_STORE_MAX_RETRIES="5"
        "#;
        lock.set_env(config);

        let config = load_object_store_config().expect("failed to load object store config");
        assert_eq!(config, default_object_store_config());
    }
}
