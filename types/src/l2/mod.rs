use ethabi::ethereum_types::U64;
pub use ola_basic_types::{bytes8::Bytes8, Address, Nonce, H256};
use ola_basic_types::{Bytes, U256};
use rlp::Rlp;

use crate::{
    api::TransactionRequest,
    request::{Eip712Meta, PaymasterParams},
    tx::{execute::Execute, primitives::PackedEthSignature},
    utils::unix_timestamp_ms,
    ExecuteTransactionCommon, InputData, Transaction, EIP_1559_TX_TYPE, EIP_712_TX_TYPE,
    OLA_RAW_TX_TYPE, PRIORITY_OPERATION_L2_TX_TYPE, PROTOCOL_UPGRADE_TX_TYPE,
};
use serde::{Deserialize, Serialize};

pub mod error;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TransactionType {
    EIP712Transaction = EIP_712_TX_TYPE as u32,
    EIP1559Transaction = EIP_1559_TX_TYPE as u32,
    OlaRawTransaction = OLA_RAW_TX_TYPE as u32,
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
        _paymaster_params: PaymasterParams,
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
                transaction_type: TransactionType::OlaRawTransaction,
                input: None,
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

    pub fn recipient_account(&self) -> Address {
        self.execute.contract_address
    }

    pub fn nonce(&self) -> Nonce {
        self.common_data.nonce
    }

    pub fn hash(&self) -> H256 {
        self.common_data.hash()
    }

    pub fn set_signature(&mut self, signatures: Vec<PackedEthSignature>) {
        let signature = signatures
            .into_iter()
            .map(|s| s.serialize_packed_without_v())
            .flat_map(|s| s.into_iter())
            .collect::<Vec<_>>();
        self.set_raw_signature(signature);
    }

    pub fn set_raw_signature(&mut self, signature: Vec<u8>) {
        self.common_data.signature = signature;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct L2TxCommonData {
    pub nonce: Nonce,
    pub initiator_address: Address,
    pub signature: Vec<u8>,
    pub transaction_type: TransactionType,
    pub input: Option<InputData>,
}

impl L2TxCommonData {
    pub fn new(
        nonce: Nonce,
        initiator_address: Address,
        signature: Vec<u8>,
        transaction_type: TransactionType,
        input: Vec<u8>,
        hash: H256,
    ) -> Self {
        let input = Some(InputData { hash, data: input });
        Self {
            nonce,
            initiator_address,
            signature,
            transaction_type,
            input,
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

    pub fn extract_chain_id(&self) -> Option<u16> {
        let bytes = self.input_data()?;
        let chain_id = match bytes.first() {
            Some(x) if *x == EIP_1559_TX_TYPE => {
                let rlp = Rlp::new(&bytes[1..]);
                rlp.val_at(0).ok()?
            }
            Some(x) if *x == EIP_712_TX_TYPE => {
                let rlp = Rlp::new(&bytes[1..]);
                rlp.val_at(6).ok()?
            }
            _ => return None,
        };
        Some(chain_id)
    }
}

impl Default for L2TxCommonData {
    fn default() -> Self {
        Self {
            nonce: Nonce(0),
            initiator_address: Address::zero(),
            signature: Default::default(),
            transaction_type: TransactionType::EIP712Transaction,
            input: Default::default(),
        }
    }
}

fn signature_to_vrs(signature: &[u8]) -> (Option<U64>, Option<U256>, Option<U256>) {
    let signature = PackedEthSignature::deserialize_packed(signature);

    if let Ok(sig) = signature {
        (
            Some(U64::from(sig.v())),
            Some(U256::from(sig.r())),
            Some(U256::from(sig.s())),
        )
    } else {
        (None, None, None)
    }
}

impl From<L2Tx> for TransactionRequest {
    fn from(tx: L2Tx) -> Self {
        let tx_type = tx.common_data.transaction_type;
        let (v, r, s) = signature_to_vrs(&tx.common_data.signature);

        let mut base_tx_req = TransactionRequest {
            nonce: U256::from(tx.common_data.nonce.0),
            from: Some(tx.common_data.initiator_address),
            to: Some(tx.recipient_account()),
            input: Bytes(tx.execute.calldata),
            v,
            r,
            s,
            raw: None,
            transaction_type: None,
            eip712_meta: None,
            chain_id: tx.common_data.extract_chain_id(),
        };
        match tx_type {
            TransactionType::EIP712Transaction => {
                base_tx_req.transaction_type = Some(U64::from(tx_type as u32));
                base_tx_req.eip712_meta = Some(Eip712Meta {
                    factory_deps: tx.execute.factory_deps,
                    custom_signature: Some(tx.common_data.signature),
                    paymaster_params: None,
                });
            }
            TransactionType::EIP1559Transaction => {
                base_tx_req.transaction_type = Some(U64::from(tx_type as u32));
            }
            TransactionType::OlaRawTransaction => {
                base_tx_req.transaction_type = Some(U64::from(tx_type as u32));
                base_tx_req.eip712_meta = Some(Eip712Meta {
                    factory_deps: tx.execute.factory_deps,
                    custom_signature: Some(tx.common_data.signature),
                    paymaster_params: None,
                });
            }
            _ => panic!("Invalid transaction type: {}", tx_type as u32),
        }
        base_tx_req
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
