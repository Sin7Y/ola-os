use std::time::SystemTime;

use ola_basic_types::U256;

use crate::system_contracts::DEPLOYMENT_NONCE_INCREMENT;

pub fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub fn decompose_full_nonce(full_nonce: U256) -> (U256, U256) {
    (
        full_nonce % DEPLOYMENT_NONCE_INCREMENT,
        full_nonce / DEPLOYMENT_NONCE_INCREMENT,
    )
}
