use ola_basic_types::{Address, Nonce, H256};

use crate::{
    request::{PaymasterParams, SerializationTransactionError, TransactionRequest},
    tx::execute::Execute,
    utils::unix_timestamp_ms,
    InputData,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Tx {
    pub execute: Execute,
    pub common_data: L2TxCommonData,
    pub timestamp: u64,
}

impl L2Tx {
    pub fn new(
        contract_address: Address,
        calldata: Vec<u8>,
        nonce: Nonce,
        initiator_address: Address,
        factory_deps: Option<Vec<Vec<u8>>>,
        paymaster_params: PaymasterParams,
    ) -> Self {
        Self {
            execute: Execute {
                contract_address,
                calldata,
                factory_deps,
            },
            common_data: L2TxCommonData {
                nonce,
                initiator_address,
                signature: Default::default(),
                input: None,
                paymaster_params,
            },
            timestamp: unix_timestamp_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct L2TxCommonData {
    pub nonce: Nonce,
    pub initiator_address: Address,
    pub signature: Vec<u8>,
    pub input: Option<InputData>,
    pub paymaster_params: PaymasterParams,
}

impl L2TxCommonData {
    pub fn new(
        nonce: Nonce,
        initiator_address: Address,
        signature: Vec<u8>,
        input: Vec<u8>,
        hash: H256,
        paymaster_params: PaymasterParams,
    ) -> Self {
        let input = Some(InputData { hash, data: input });
        Self {
            nonce,
            initiator_address,
            signature,
            input,
            paymaster_params,
        }
    }
}

impl Default for L2TxCommonData {
    fn default() -> Self {
        Self {
            nonce: Nonce(0),
            initiator_address: Address::zero(),
            signature: Default::default(),
            input: Default::default(),
            paymaster_params: Default::default(),
        }
    }
}
