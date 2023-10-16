use ola_basic_types::U256;

pub const TX_NONCE_INCREMENT: U256 = U256([1, 0, 0, 0]); // 1
pub const DEPLOYMENT_NONCE_INCREMENT: U256 = U256([0, 0, 1, 0]); // 2^128
