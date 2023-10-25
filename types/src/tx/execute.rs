use ola_basic_types::Address;
use serde::{Deserialize, Serialize};

// TODO: @Pierre
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Execute {
    pub contract_address: Address,
    pub calldata: Vec<u8>,
    pub factory_deps: Option<Vec<Vec<u8>>>,
}

impl Execute {
    pub fn factory_deps_length(&self) -> usize {
        self.factory_deps
            .as_ref()
            .map(|deps| deps.len())
            .unwrap_or_default()
    }
}
