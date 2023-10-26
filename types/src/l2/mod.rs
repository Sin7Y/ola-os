use ola_basic_types::{Address, Nonce, H256};

use crate::{
    request::{PaymasterParams, SerializationTransactionError, TransactionRequest},
    tx::execute::Execute,
    utils::unix_timestamp_ms,
    ExecuteTransactionCommon, InputData, Transaction, PRIORITY_OPERATION_L2_TX_TYPE,
    PROTOCOL_UPGRADE_TX_TYPE,
};
use serde::{Deserialize, Serialize};

pub mod error;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TransactionType {
    EIP1559Transaction = 0,
    PriorityOpTransaction = PRIORITY_OPERATION_L2_TX_TYPE as u32,
    ProtocolUpgradeTransaction = PROTOCOL_UPGRADE_TX_TYPE as u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Tx {
    pub execute: Execute,
    pub common_data: L2TxCommonData,
    pub received_timestamp_ms: u64,
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
            received_timestamp_ms: unix_timestamp_ms(),
        }
    }

    pub fn set_input(&mut self, data: Vec<u8>, hash: H256) {
        self.common_data.set_input(data, hash)
    }

    pub fn initiator_account(&self) -> Address {
        self.common_data.initiator_address
    }

    pub fn nonce(&self) -> Nonce {
        self.common_data.nonce
    }

    pub fn hash(&self) -> H256 {
        self.common_data.hash()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct L2TxCommonData {
    pub nonce: Nonce,
    pub initiator_address: Address,
    pub signature: Vec<u8>,
    pub transaction_type: TransactionType,
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

    pub fn set_input(&mut self, input: Vec<u8>, hash: H256) {
        self.input = Some(InputData { hash, data: input })
    }

    pub fn hash(&self) -> H256 {
        self.input
            .as_ref()
            .expect("Transaction must have input data")
            .hash
    }

    pub fn input_data(&self) -> Option<&[u8]> {
        self.input.as_ref().map(|input| &*input.data)
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

impl From<L2Tx> for Transaction {
    fn from(tx: L2Tx) -> Self {
        let L2Tx {
            execute,
            common_data,
            received_timestamp_ms,
        } = tx;
        Self {
            common_data: ExecuteTransactionCommon::L2(common_data),
            execute,
            received_timestamp_ms,
        }
    }
}
