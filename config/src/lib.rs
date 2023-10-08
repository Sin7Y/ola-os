use serde::de::DeserializeOwned;

pub mod api;
pub mod sequencer;
pub mod utils;
pub mod database;

const BYTES_IN_MB: usize = 1_024 * 1_024;

pub fn envy_load<T: DeserializeOwned>(name: &str, prefix: &str) -> T {
    envy_try_load(prefix).unwrap_or_else(|_| {
        panic!("Cannot load config <{}>: {}", name, prefix);
    })
}

pub fn envy_try_load<T: DeserializeOwned>(prefix: &str) -> Result<T, envy::Error> {
    envy::prefixed(prefix).from_env()
}