use std::path::Path;

use environment::Environment;
use serde::de::DeserializeOwned;

pub mod api;
pub mod database;
pub mod environment;
pub mod sequencer;
pub mod utils;

const BYTES_IN_MB: usize = 1_024 * 1_024;

pub fn envy_load<T: DeserializeOwned>(name: &str, prefix: &str) -> T {
    envy_try_load(prefix).unwrap_or_else(|_| {
        panic!("Cannot load config <{}>: {}", name, prefix);
    })
}

pub fn envy_try_load<T: DeserializeOwned>(prefix: &str) -> Result<T, envy::Error> {
    envy::prefixed(prefix).from_env()
}

pub fn load_config<P: AsRef<Path>, T: DeserializeOwned>(path: P) -> Result<T, config::ConfigError> {
    let mut settings = config::Config::default();
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");
    let configuration_directory = base_path.join(path);
    // Read the "default" configuration file
    settings.merge(config::File::from(configuration_directory.join("base")).required(true))?;
    // Detect the running environment.
    // Default to `local` if unspecified.
    let environment: Environment = std::env::var("OLAOS_APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse OLAOS_APP_ENVIRONMENT.");
    // Layer on the environment-specific values.
    settings.merge(
        config::File::from(configuration_directory.join(environment.as_str())).required(true),
    )?;
    // Add in settings from environment variables (with a prefix of APP and '__' as separator)
    // E.g. `APP_APPLICATION__PORT=5001 would set `Settings.application.port`
    settings.merge(config::Environment::with_prefix("OLAOS").separator("__"))?;
    // Try to convert the configuration values it read into
    // our Settings type
    settings.try_into()
}
