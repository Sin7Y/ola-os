use crate::{
    l2::L2Tx,
    tx::primitives::{EIP712TypedStructure, Eip712Domain, PackedEthSignature, StructBuilder},
    EIP_1559_TX_TYPE, EIP_712_TX_TYPE,
};
use ethabi::ethereum_types::U64;
use ola_basic_types::{Address, L2ChainId, Nonce, H256};
use ola_utils::{
    bytecode::{hash_bytecode, validate_bytecode, InvalidBytecodeError},
    hash::hash_bytes,
};
use rlp::{DecoderError, Rlp};
use serde::{Deserialize, Serialize};
use thiserror::Error;
pub use web3::types::{Bytes, U256};

#[derive(Debug, Error, PartialEq)]
pub enum SerializationTransactionError {
    #[error("transaction type is not supported")]
    UnknownTransactionFormat,
    #[error("to address is null")]
    ToAddressIsNull,
    #[error("invalid paymaster params")]
    InvalidPaymasterParams,
    #[error("nonce is too big")]
    TooBigNonce,
    #[error("factory dependency #{0} is invalid: {1}")]
    InvalidFactoryDependencies(usize, InvalidBytecodeError),
    #[error("decodeRlpError {0}")]
    DecodeRlpError(#[from] DecoderError),
    #[error("access lists are not supported")]
    AccessListsNotSupported,
    #[error("wrong chain id {}", .0.unwrap_or_default())]
    WrongChainId(Option<u16>),
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaymasterParams {
    pub paymaster: Address,
    pub paymaster_input: Vec<u8>,
}

impl PaymasterParams {
    fn from_vector(value: Vec<Vec<u8>>) -> Result<Option<Self>, SerializationTransactionError> {
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
    /// ECDSA recovery id
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<U64>,
    /// ECDSA signature r, 32 bytes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r: Option<U256>,
    /// ECDSA signature s, 32 bytes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<U256>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Bytes>,
    /// Transaction type, Some(1) for AccessList transaction, None for Legacy
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<U64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eip712_meta: Option<Eip712Meta>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<u16>,
}

impl EIP712TypedStructure for TransactionRequest {
    const TYPE_NAME: &'static str = "Transaction";

    fn build_structure<BUILDER: StructBuilder>(&self, builder: &mut BUILDER) {
        let meta = self
            .eip712_meta
            .as_ref()
            .expect("We can sign transaction only with meta");
        builder.add_member(
            "txType",
            &self
                .transaction_type
                .map(|x| U256::from(x.as_u64()))
                .unwrap_or_else(|| U256::from(EIP_712_TX_TYPE)),
        );
        builder.add_member(
            "from",
            &U256::from(
                self.from
                    .expect("We can only sign transactions with known sender")
                    .as_bytes(),
            ),
        );
        builder.add_member("to", &U256::from(self.to.unwrap_or_default().as_bytes()));

        builder.add_member(
            "paymaster",
            &U256::from(self.get_paymaster().unwrap_or_default().as_bytes()),
        );
        builder.add_member("nonce", &self.nonce);
        builder.add_member("data", &self.input.0.as_slice());

        let factory_dep_hashes: Vec<_> = self
            .get_factory_deps()
            .into_iter()
            .map(|dep| hash_bytecode(&dep))
            .collect();
        builder.add_member("factoryDeps", &factory_dep_hashes.as_slice());

        builder.add_member(
            "paymasterInput",
            &self.get_paymaster_input().unwrap_or_default().as_slice(),
        );
    }
}

impl TransactionRequest {
    pub fn is_eip712_tx(&self) -> bool {
        Some(EIP_712_TX_TYPE.into()) == self.transaction_type
    }

    pub fn get_custom_signature(&self) -> Option<Vec<u8>> {
        self.eip712_meta
            .as_ref()
            .and_then(|meta| meta.custom_signature.as_ref())
            .cloned()
    }

    pub fn get_paymaster(&self) -> Option<Address> {
        self.eip712_meta
            .clone()
            .and_then(|meta| meta.paymaster_params)
            .map(|params| params.paymaster)
    }

    pub fn get_paymaster_input(&self) -> Option<Vec<u8>> {
        self.eip712_meta
            .clone()
            .and_then(|meta| meta.paymaster_params)
            .map(|params| params.paymaster_input)
    }

    pub fn get_factory_deps(&self) -> Vec<Vec<u8>> {
        self.eip712_meta
            .clone()
            .and_then(|meta| meta.factory_deps)
            .unwrap_or_default()
    }

    pub fn from_bytes(
        bytes: &[u8],
        chain_id: u16,
    ) -> Result<(Self, H256), SerializationTransactionError> {
        // TODO:
        let tx = Self::default();
        let rlp;
        let mut tx = match bytes.first() {
            Some(&EIP_1559_TX_TYPE) => {
                rlp = Rlp::new(&bytes[1..]);
                if rlp.item_count()? != 12 {
                    return Err(SerializationTransactionError::DecodeRlpError(
                        DecoderError::RlpIncorrectListLen,
                    ));
                }
                if let Ok(access_list_rlp) = rlp.at(8) {
                    if access_list_rlp.item_count()? > 0 {
                        return Err(SerializationTransactionError::AccessListsNotSupported);
                    }
                }
                let tx_chain_id = rlp.val_at(0).ok();
                if tx_chain_id != Some(chain_id) {
                    return Err(SerializationTransactionError::WrongChainId(tx_chain_id));
                }
                Self {
                    chain_id: tx_chain_id,
                    v: Some(rlp.val_at(9)?),
                    r: Some(rlp.val_at(10)?),
                    s: Some(rlp.val_at(11)?),
                    raw: Some(Bytes(rlp.as_raw().to_vec())),
                    transaction_type: Some(EIP_1559_TX_TYPE.into()),
                    ..Self::decode_eip1559_fields(&rlp, 1)?
                }
            }
            Some(&EIP_712_TX_TYPE) => {
                rlp = Rlp::new(&bytes[1..]);
                if rlp.item_count()? != 16 {
                    return Err(SerializationTransactionError::DecodeRlpError(
                        DecoderError::RlpIncorrectListLen,
                    ));
                }
                let tx_chain_id = rlp.val_at(10).ok();
                if tx_chain_id.is_some() && tx_chain_id != Some(chain_id) {
                    return Err(SerializationTransactionError::WrongChainId(tx_chain_id));
                }

                Self {
                    v: Some(rlp.val_at(7)?),
                    r: Some(rlp.val_at(8)?),
                    s: Some(rlp.val_at(9)?),
                    eip712_meta: Some(Eip712Meta {
                        factory_deps: rlp.list_at(13).ok(),
                        custom_signature: rlp.val_at(14).ok(),
                        paymaster_params: if let Ok(params) = rlp.list_at(15) {
                            PaymasterParams::from_vector(params)?
                        } else {
                            None
                        },
                    }),
                    chain_id: tx_chain_id,
                    transaction_type: Some(EIP_712_TX_TYPE.into()),
                    from: Some(rlp.val_at(11)?),
                    ..Self::decode_eip1559_fields(&rlp, 0)?
                }
            }
            _ => return Err(SerializationTransactionError::UnknownTransactionFormat),
        };
        let factory_deps_ref = tx
            .eip712_meta
            .as_ref()
            .and_then(|m| m.factory_deps.as_ref());
        if let Some(deps) = factory_deps_ref {
            validate_factory_deps(deps)?;
        }
        tx.raw = Some(Bytes(bytes.to_vec()));
        let default_signed_message = tx.get_default_signed_message(chain_id);
        tx.from = match tx.from {
            Some(_) => tx.from,
            // FIXME: can from unset?
            None => panic!("from must be set"),
        };
        let hash = if tx.is_eip712_tx() {
            let digest = [
                default_signed_message.as_bytes(),
                &hash_bytes(&tx.get_signature()?).as_bytes(),
            ]
            .concat();
            hash_bytecode(&digest)
        } else {
            hash_bytecode(bytes)
        };

        Ok((tx, hash))
    }

    pub fn get_nonce_checked(&self) -> Result<Nonce, SerializationTransactionError> {
        if self.nonce <= U256::from(u32::MAX) {
            Ok(Nonce(self.nonce.as_u32()))
        } else {
            Err(SerializationTransactionError::TooBigNonce)
        }
    }

    fn decode_eip1559_fields(rlp: &Rlp, offset: usize) -> Result<Self, DecoderError> {
        Ok(Self {
            nonce: rlp.val_at(offset)?,
            to: rlp.val_at(offset + 4).ok(),
            input: Bytes(rlp.val_at(offset + 6)?),
            ..Default::default()
        })
    }

    fn get_default_signed_message(&self, chain_id: u16) -> H256 {
        // TODO:
        if self.is_eip712_tx() {
            PackedEthSignature::typed_data_to_signed_bytes(
                &Eip712Domain::new(L2ChainId(chain_id)),
                self,
            )
        } else {
            // let mut rlp_stream = RlpStream::new();
            // self.rlp(&mut rlp_stream, chain_id, None);
            // let mut data = rlp_stream.out().to_vec();
            // if let Some(tx_type) = self.transaction_type {
            //     data.insert(0, tx_type.as_u64() as u8);
            // }
            // PackedEthSignature::message_to_signed_bytes(&data)
            panic!("other tx type is not supported!");
        }
    }

    pub fn get_signature(&self) -> Result<Vec<u8>, SerializationTransactionError> {
        let custom_signature = self.get_custom_signature();
        if let Some(custom_sig) = custom_signature {
            if !custom_sig.is_empty() {
                // There was a custom signature supplied, it overrides
                // the v/r/s signature
                return Ok(custom_sig);
            }
        }
        panic!("packed_eth_signature is unsupported");
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Eip712Meta {
    #[serde(default)]
    pub factory_deps: Option<Vec<Vec<u8>>>,
    pub custom_signature: Option<Vec<u8>>,
    pub paymaster_params: Option<PaymasterParams>,
}

impl L2Tx {
    pub fn from_request(
        request: TransactionRequest,
        _max_tx_size: usize,
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
            request.input.0.clone(),
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
