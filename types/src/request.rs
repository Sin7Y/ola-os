use crate::l2::L2Tx;
use ola_basic_types::{Nonce, Address};
use serde::{Serialize, Deserialize};
use thiserror::Error;
use web3::types::{U256, Bytes};

#[derive(Debug, Error, PartialEq)]
pub enum SerializationTransactionError {
    #[error("to address is null")]
    ToAddressIsNull,
    #[error("invalid paymaster params")]
    InvalidPaymasterParams,
    #[error("nonce is too big")]
    TooBigNonce,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaymasterParams {
    pub paymaster: Address,
    pub paymaster_input: Vec<u8>,
}

impl PaymasterParams {
    fn new(value: Vec<Vec<u8>>) -> Result<Option<Self>, SerializationTransactionError> {
        if value.is_empty() {
            return Ok(None);
        }
        if value.len() != 2 || value[0].len() != 20 {
            return Err(SerializationTransactionError::InvalidPaymasterParams);
        }
        let result = Some(Self {
            paymaster: Address::from_slice(&value[0]),
            paymaster_input: value[1].clone(),
        });
        Ok(result)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRequest {
    pub nonce: U256,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Address>,
    pub to: Option<Address>,
    pub input: Bytes,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Bytes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eip712_meta: Option<Eip712Meta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<u16>,
}

impl TransactionRequest {
    pub fn from_bytes(bytes: &[u8], chain_id: u16) -> Result<Self, SerializationTransactionError> {
        let tx = Self::default();
        Ok(tx)
    }

    pub fn get_nonce_checked(&self) -> Result<Nonce, SerializationTransactionError> {
        if self.nonce <= U256::from(u32::MAX) {
            Ok(Nonce(self.nonce.as_u32()))
        } else {
            Err(SerializationTransactionError::TooBigNonce)
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Eip712Meta {
    pub gas_per_pubdata: U256,
    #[serde(default)]
    pub factory_deps: Option<Vec<Vec<u8>>>,
    pub custom_signature: Option<Vec<u8>>,
    pub paymaster_params: Option<PaymasterParams>,
}

impl L2Tx {
    pub fn from_request(request: TransactionRequest, max_tx_size: usize) -> Result<Self, SerializationTransactionError> {
        let nonce = request.get_nonce_checked()?;
        let (factory_deps, paymaster_params) = request.eip712_meta
            .map(|eip712_meta| (eip712_meta.factory_deps, eip712_meta.paymaster_params))
            .unwrap_or_default();

        let tx = L2Tx::new(
            request.to.ok_or(SerializationTransactionError::ToAddressIsNull)?,
            request.input.0.clone(),
            nonce,
            request.from.unwrap_or_default(),
            factory_deps,
            paymaster_params.unwrap_or_default(),
        );
        Ok(tx)
    }
}