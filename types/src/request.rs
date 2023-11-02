use crate::l2::L2Tx;
use ola_basic_types::{bytes8::Bytes8, Address, Nonce, H256};
use ola_utils::bytecode::{validate_bytecode, InvalidBytecodeError};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use web3::types::{Bytes, U256};

#[derive(Debug, Error, PartialEq)]
pub enum SerializationTransactionError {
    #[error("to address is null")]
    ToAddressIsNull,
    #[error("invalid paymaster params")]
    InvalidPaymasterParams,
    #[error("nonce is too big")]
    TooBigNonce,
    #[error("factory dependency #{0} is invalid: {1}")]
    InvalidFactoryDependencies(usize, InvalidBytecodeError),
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
    pub input: Bytes8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Bytes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eip712_meta: Option<Eip712Meta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<u16>,
}

impl TransactionRequest {
    pub fn from_bytes(
        bytes: &[u8],
        chain_id: u16,
    ) -> Result<(Self, H256), SerializationTransactionError> {
        // TODO:
        let mut tx = Self::default();
        // let factory_deps_ref = tx
        //     .eip712_meta
        //     .as_ref()
        //     .and_then(|m| m.factory_deps.as_ref());
        // if let Some(deps) = factory_deps_ref {
        //     validate_factory_deps(deps)?;
        // }
        // tx.raw = Some(Bytes(bytes.to_vec()));
        // let default_signed_message = tx.get_default_signed_message(chain_id);
        // tx.from = match tx.from {
        //     Some(_) => tx.from,
        //     // FIXME: can from unset?
        //     None => panic!("from must be set"),
        // };
        // TODO: hash = default_signed_message + keccak(signature)
        let hash = H256::default();

        Ok((tx, hash))
    }

    pub fn get_nonce_checked(&self) -> Result<Nonce, SerializationTransactionError> {
        if self.nonce <= U256::from(u32::MAX) {
            Ok(Nonce(self.nonce.as_u64()))
        } else {
            Err(SerializationTransactionError::TooBigNonce)
        }
    }

    fn get_default_signed_message(&self, chain_id: u64) -> H256 {
        // TODO:
        H256::default()
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
    pub fn from_request(
        request: TransactionRequest,
        max_tx_size: usize,
    ) -> Result<Self, SerializationTransactionError> {
        let nonce = request.get_nonce_checked()?;
        let (factory_deps, paymaster_params) = request
            .eip712_meta
            .map(|eip712_meta| (eip712_meta.factory_deps, eip712_meta.paymaster_params))
            .unwrap_or_default();

        let tx = L2Tx::new(
            request
                .to
                .ok_or(SerializationTransactionError::ToAddressIsNull)?,
            request.input.clone(),
            nonce,
            request.from.unwrap_or_default(),
            factory_deps,
            paymaster_params.unwrap_or_default(),
        );
        Ok(tx)
    }
}

pub fn validate_factory_deps(
    factory_deps: &[Vec<u8>],
) -> Result<(), SerializationTransactionError> {
    for (i, dep) in factory_deps.iter().enumerate() {
        validate_bytecode(dep)
            .map_err(|err| SerializationTransactionError::InvalidFactoryDependencies(i, err))?;
    }

    Ok(())
}
