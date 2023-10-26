use ola_basic_types::{Address, H160};

pub mod contracts;
pub mod crypto;
pub mod ethereum;
pub mod system_context;
pub mod trusted_slots;

pub const MAX_NEW_FACTORY_DEPS: usize = 32;

// FIXME:
pub const NONCE_HOLDER_ADDRESS: Address = H160([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x80, 0x03,
]);
