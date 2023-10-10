use ola_basic_types::Address;
use serde::{Deserialize, Serialize};
use web3::types::H160;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Execute {
    pub contract_address: Address,
    pub calldata: Vec<u8>,
    pub factory_deps: Option<Vec<Vec<u8>>>,
}
