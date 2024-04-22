use ola_types::{L2ChainId, U256};

#[derive(Debug, Clone)]
pub struct NetNamespace {
    ola_network_id: L2ChainId,
}

impl NetNamespace {
    pub fn new(ola_network_id: L2ChainId) -> Self {
        Self { ola_network_id }
    }

    pub fn version_impl(&self) -> String {
        self.ola_network_id.0.to_string()
    }

    pub fn peer_count_impl(&self) -> U256 {
        0.into()
    }

    pub fn is_listening_impl(&self) -> bool {
        false
    }
}
