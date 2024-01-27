use crate::{
    l2::{L2Tx, TransactionType},
    tx::primitives::{EIP712TypedStructure, Eip712Domain, PackedEthSignature, StructBuilder},
    EIP_1559_TX_TYPE, EIP_712_TX_TYPE, OLA_RAW_TX_TYPE,
};
use ethabi::ethereum_types::U64;
use ola_basic_types::{Address, L2ChainId, Nonce, H256};
use ola_utils::{
    address_to_h256,
    bytecode::{hash_bytecode, validate_bytecode, InvalidBytecodeError},
    bytes_to_u64s, h256_to_u64_array,
    hash::hash_bytes,
    u64s_to_bytes,
};
use rlp::{DecoderError, Rlp, RlpStream};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use web3::types::AccessList;
pub use web3::types::{Bytes, U256};

#[derive(Debug, Error, PartialEq)]
pub enum SerializationTransactionError {
    #[error("transaction type is not supported")]
    UnknownTransactionFormat,
    #[error("from address is null")]
    FromAddressIsNull,
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
    #[error("oversized data. max: {0}; actual: {1}")]
    OversizedData(usize, usize),
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
        if value.len() != 2 || value[0].len() != 32 {
            return Err(SerializationTransactionError::InvalidPaymasterParams);
        }
        let result = Some(Self {
            paymaster: Address::from_slice(&value[0]),
            paymaster_input: value[1].clone(),
        });
        Ok(result)
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest {
    /// Sender address (None for arbitrary address)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Address>,
    /// To address (None allowed for eth_estimateGas)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Address>,
    /// Data (None for empty data)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Bytes>,
    /// Nonce
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<U256>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub transaction_type: Option<U64>,
    /// Eip712 meta
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eip712_meta: Option<Eip712Meta>,
}

impl CallRequest {
    /// Function to return a builder for a Call Request
    pub fn builder() -> CallRequestBuilder {
        CallRequestBuilder::default()
    }
}

/// Call Request Builder
#[derive(Clone, Debug, Default)]
pub struct CallRequestBuilder {
    call_request: CallRequest,
}

impl CallRequestBuilder {
    /// Set sender address (None for arbitrary address)
    pub fn from(mut self, from: Address) -> Self {
        self.call_request.from = Some(from);
        self
    }

    /// Set to address (None allowed for eth_estimateGas)
    pub fn to(mut self, to: Address) -> Self {
        self.call_request.to = Some(to);
        self
    }

    /// Set data (None for empty data)
    pub fn data(mut self, data: Bytes) -> Self {
        self.call_request.data = Some(data);
        self
    }

    /// Set transaction type, Some(1) for AccessList transaction, None for Legacy
    pub fn transaction_type(mut self, transaction_type: U64) -> Self {
        self.call_request.transaction_type = Some(transaction_type);
        self
    }

    /// Set meta
    pub fn eip712_meta(mut self, eip712_meta: Eip712Meta) -> Self {
        self.call_request.eip712_meta = Some(eip712_meta);
        self
    }

    /// build the Call Request
    pub fn build(&self) -> CallRequest {
        self.call_request.clone()
    }
}

impl From<L2Tx> for CallRequest {
    fn from(tx: L2Tx) -> Self {
        let mut meta = Eip712Meta {
            factory_deps: None,
            custom_signature: Some(tx.common_data.signature.clone()),
            paymaster_params: None,
        };
        meta.factory_deps = tx.execute.factory_deps.clone();
        let mut request = CallRequestBuilder::default()
            .from(tx.initiator_account())
            .transaction_type(U64::from(tx.common_data.transaction_type as u32))
            .to(tx.execute.contract_address)
            .data(Bytes(tx.execute.calldata.clone()))
            .eip712_meta(meta)
            .build();

        request.transaction_type = match tx.common_data.transaction_type {
            TransactionType::EIP712Transaction => Some(U64::from(EIP_712_TX_TYPE)),
            TransactionType::EIP1559Transaction => Some(U64::from(EIP_1559_TX_TYPE)),
            _ => Some(U64::from(OLA_RAW_TX_TYPE)),
        };
        request
    }
}

impl From<CallRequest> for TransactionRequest {
    fn from(call_request: CallRequest) -> Self {
        TransactionRequest {
            nonce: call_request.nonce.unwrap_or_default(),
            from: call_request.from,
            to: call_request.to,
            input: call_request.data.unwrap_or_default(),
            transaction_type: call_request.transaction_type,
            eip712_meta: call_request.eip712_meta,
            ..Default::default()
        }
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
    pub fn is_ola_raw_tx(&self) -> bool {
        Some(OLA_RAW_TX_TYPE.into()) == self.transaction_type
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

    pub fn into_signed_bytes(&self) -> Vec<u8> {
        let meta = self
            .eip712_meta
            .as_ref()
            .expect("We can sign transaction only with meta");
        if &self.input.0.len() % 8 != 0 {
            panic!("Transaction data must be 8-byte aligned");
        }
        match &meta.paymaster_params {
            Some(params) => {
                if params.paymaster_input.len() % 8 != 0 {
                    panic!("Paymaster input must be 8-byte aligned");
                }
            }
            None => {}
        }
        match &self.chain_id {
            Some(_) => {}
            None => {
                panic!("Chain id must be set when perform sign");
            }
        }
        let chain_id = self.chain_id.unwrap().clone() as u64;

        let transaction_type = &self
            .transaction_type
            .unwrap_or_else(|| U64([OLA_RAW_TX_TYPE as u64]))
            .0[0];
        let nonce = &self.nonce.as_u64();

        let from = h256_to_u64_array(
            &self
                .from
                .expect("We can only sign transactions with known sender"),
        )
        .to_vec();

        let input = bytes_to_u64s(self.input.clone().0);
        let pos_biz_calldata_start = 8;
        let biz_calldata_len = input.get(pos_biz_calldata_start).unwrap_or(&0);
        let pos_biz_calldata_end = pos_biz_calldata_start + *biz_calldata_len as usize + 1;
        let biz_input = input[pos_biz_calldata_start..pos_biz_calldata_end].to_vec();
        let biz_addr = input[4..8].to_vec();

        let have_paymaster: u64 = if meta.paymaster_params.is_some() {
            1
        } else {
            0
        };
        let paymaster_address = if have_paymaster == 1 {
            let addr = meta
                .paymaster_params
                .as_ref()
                .expect("Missing paymaster params")
                .paymaster;
            Some(h256_to_u64_array(&addr).to_vec())
        } else {
            None
        };
        let paymaster_input_len = if have_paymaster == 1 {
            let len = meta
                .paymaster_params
                .as_ref()
                .expect("Missing paymaster params")
                .paymaster_input
                .len();
            Some((len / 8) as u64)
        } else {
            None
        };
        let paymaster_input = if have_paymaster == 1 {
            let input = &meta
                .paymaster_params
                .as_ref()
                .expect("Missing paymaster params")
                .paymaster_input;
            Some(bytes_to_u64s(input.to_vec()))
        } else {
            None
        };

        let mut data = vec![
            vec![chain_id, *transaction_type, *nonce],
            from,
            biz_addr,
            biz_input,
        ]
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
        match paymaster_address {
            Some(address) => {
                data.extend(address);
            }
            None => {}
        }
        match paymaster_input_len {
            Some(len) => {
                data.push(len);
            }
            None => {}
        }
        match paymaster_input {
            Some(input) => {
                data.extend(input);
            }
            None => {}
        }
        u64s_to_bytes(&data)
    }

    pub fn from_bytes(
        bytes: &[u8],
        chain_id: u16,
    ) -> Result<(Self, H256), SerializationTransactionError> {
        // TODO:
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
            Some(&EIP_712_TX_TYPE) | Some(&OLA_RAW_TX_TYPE) => {
                rlp = Rlp::new(&bytes[1..]);
                if rlp.item_count()? != 11 {
                    return Err(SerializationTransactionError::DecodeRlpError(
                        DecoderError::RlpIncorrectListLen,
                    ));
                }
                let tx_chain_id = rlp.val_at(6).ok();
                if tx_chain_id.is_some() && tx_chain_id != Some(chain_id) {
                    return Err(SerializationTransactionError::WrongChainId(tx_chain_id));
                }

                Self {
                    v: Some(rlp.val_at(3)?),
                    r: Some(rlp.val_at(4)?),
                    s: Some(rlp.val_at(5)?),
                    eip712_meta: Some(Eip712Meta {
                        factory_deps: rlp.list_at(8).ok(),
                        custom_signature: rlp.val_at(9).ok(),
                        paymaster_params: if let Ok(params) = rlp.list_at(10) {
                            PaymasterParams::from_vector(params)?
                        } else {
                            None
                        },
                    }),
                    chain_id: tx_chain_id,
                    transaction_type: Some(OLA_RAW_TX_TYPE.into()),
                    from: Some(rlp.val_at(7)?),
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
            None => return Err(SerializationTransactionError::FromAddressIsNull),
        };
        let hash = if tx.is_eip712_tx() || tx.is_ola_raw_tx() {
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

    pub fn get_signed_bytes(&self, signature: &PackedEthSignature, chain_id: u16) -> Vec<u8> {
        let mut rlp = RlpStream::new();
        self.rlp(&mut rlp, chain_id, Some(signature));
        let mut data = rlp.out().to_vec();
        if let Some(tx_type) = self.transaction_type {
            data.insert(0, tx_type.as_u64() as u8);
        }
        data
    }

    pub fn rlp(&self, rlp: &mut RlpStream, chain_id: u16, signature: Option<&PackedEthSignature>) {
        rlp.begin_unbounded_list();

        match self.transaction_type {
            // EIP-1559 (0x02)
            Some(x) if x == EIP_1559_TX_TYPE.into() => {
                rlp.append(&chain_id);
                rlp.append(&self.nonce);
                rlp_opt(rlp, &self.to);
                rlp.append(&self.input.0);
                // access_list_rlp(rlp, &self.access_list);
            }
            // EIP-712
            Some(x) if x == EIP_712_TX_TYPE.into() => {
                rlp.append(&self.nonce);
                rlp_opt(rlp, &self.to);
                rlp.append(&self.input.0);
            }
            Some(x) if x == OLA_RAW_TX_TYPE.into() => {
                rlp.append(&self.nonce);
                rlp_opt(rlp, &self.to);
                rlp.append(&self.input.0);
            }
            _ => unreachable!("Unknown tx type"),
        }

        if let Some(signature) = signature {
            let v = signature.v();
            rlp.append(&U256::from_big_endian(&[v]));
            rlp.append(&U256::from_big_endian(signature.r()));
            rlp.append(&U256::from_big_endian(signature.s()));
        }

        if self.is_eip712_tx() || self.is_ola_raw_tx() {
            rlp.append(&chain_id);
            rlp_opt(rlp, &self.from);
            if let Some(meta) = &self.eip712_meta {
                meta.rlp_append(rlp);
            }
        }

        rlp.finalize_unbounded_list();
    }

    fn decode_eip1559_fields(rlp: &Rlp, offset: usize) -> Result<Self, DecoderError> {
        Ok(Self {
            nonce: rlp.val_at(offset)?,
            to: rlp.val_at(offset + 1).ok(),
            input: Bytes(rlp.val_at(offset + 2)?),
            ..Default::default()
        })
    }

    fn get_default_signed_message(&self, chain_id: u16) -> H256 {
        let data = self.into_signed_bytes();
        PackedEthSignature::message_to_signed_bytes(&data)
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
        Ok([0; 32].to_vec())
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

impl Eip712Meta {
    pub fn rlp_append(&self, rlp: &mut RlpStream) {
        if let Some(factory_deps) = &self.factory_deps {
            rlp.begin_list(factory_deps.len());
            for dep in factory_deps.iter() {
                rlp.append(&dep.as_slice());
            }
        } else {
            rlp.begin_list(0);
        }

        rlp_opt(rlp, &self.custom_signature);

        if let Some(paymaster_params) = &self.paymaster_params {
            rlp.begin_list(2);
            rlp.append(&paymaster_params.paymaster.as_bytes());
            rlp.append(&paymaster_params.paymaster_input);
        } else {
            rlp.begin_list(0);
        }
    }
}

impl L2Tx {
    pub fn from_request(
        request: TransactionRequest,
        max_tx_size: usize,
    ) -> Result<Self, SerializationTransactionError> {
        let nonce = request.get_nonce_checked()?;

        let raw_signature = request.get_signature().unwrap_or_else(|_| [0; 32].to_vec());
        let (factory_deps, paymaster_params) = request
            .eip712_meta
            .map(|eip712_meta| (eip712_meta.factory_deps, eip712_meta.paymaster_params))
            .unwrap_or_default();

        if let Some(deps) = factory_deps.as_ref() {
            validate_factory_deps(deps)?;
        }

        let contrace_address = request
            .to
            .ok_or(SerializationTransactionError::ToAddressIsNull)?;
        let mut tx = L2Tx::new(
            contrace_address,
            request.input.0.clone(),
            nonce,
            request.from.unwrap_or_default(),
            factory_deps,
            paymaster_params.unwrap_or_default(),
        );
        tx.common_data.transaction_type = match request.transaction_type.map(|t| t.as_u64() as u8) {
            Some(EIP_712_TX_TYPE) => TransactionType::EIP712Transaction,
            Some(EIP_1559_TX_TYPE) => TransactionType::EIP1559Transaction,
            Some(OLA_RAW_TX_TYPE) => TransactionType::OlaRawTransaction,
            _ => TransactionType::OlaRawTransaction,
        };
        tx.set_raw_signature(raw_signature);

        tx.check_encoded_size(max_tx_size)?;

        Ok(tx)
    }

    fn check_encoded_size(&self, _max_tx_size: usize) -> Result<(), SerializationTransactionError> {
        // TODO:
        // let tx_size = self.abi_encoding_len() * 32;
        // if tx_size > max_tx_size {
        //     return Err(SerializationTransactionError::OversizedData(
        //         max_tx_size,
        //         tx_size,
        //     ));
        // };
        Ok(())
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

fn rlp_opt<T: rlp::Encodable>(rlp: &mut RlpStream, opt: &Option<T>) {
    if let Some(inner) = opt {
        rlp.append(inner);
    } else {
        rlp.append(&"");
    }
}

fn access_list_rlp(rlp: &mut RlpStream, access_list: &Option<AccessList>) {
    if let Some(access_list) = access_list {
        rlp.begin_list(access_list.len());
        for item in access_list {
            rlp.begin_list(2);
            rlp.append(&item.address);
            rlp.append_list(&item.storage_keys);
        }
    } else {
        rlp.begin_list(0);
    }
}

#[cfg(test)]
mod tests {
    use ethabi::ethereum_types::U64;
    use ola_basic_types::{Address, Bytes, L2ChainId, H256, U256};
    use rlp::RlpStream;

    use crate::{
        api::TransactionRequest,
        request::{Eip712Meta, PaymasterParams},
        tx::primitives::{Eip712Domain, PackedEthSignature},
        EIP_712_TX_TYPE, OLA_RAW_TX_TYPE,
    };

    #[test]
    fn decode_eip712_with_meta() {
        let private_key = H256::random();
        let address = PackedEthSignature::address_from_private_key(&private_key).unwrap();

        let mut tx = TransactionRequest {
            nonce: U256::from(0u32),
            to: Some(Address::random()),
            from: Some(address),
            input: Bytes::from(vec![1, 2, 3, 4, 5, 6, 7, 8]),
            transaction_type: Some(U64::from(OLA_RAW_TX_TYPE)),
            eip712_meta: Some(Eip712Meta {
                factory_deps: Some(vec![vec![2; 32]]),
                custom_signature: Some(vec![1; 32]),
                paymaster_params: Some(PaymasterParams {
                    paymaster: Default::default(),
                    paymaster_input: vec![],
                }),
            }),
            chain_id: Some(1027),
            ..Default::default()
        };

        let msg = PackedEthSignature::typed_data_to_signed_bytes(
            &Eip712Domain::new(L2ChainId(1027)),
            &tx,
        );
        let signature = PackedEthSignature::sign_raw(&private_key, &msg).unwrap();

        tx.v = Some(U64([signature.v() as u64; 1]));
        tx.r = Some(U256::from_big_endian(&signature.r()));
        tx.s = Some(U256::from_big_endian(&signature.s()));
        let data = tx.get_signed_bytes(&signature, 1027);
        tx.raw = Some(Bytes(data.clone()));

        let (tx2, _) = TransactionRequest::from_bytes(&data, 1027).unwrap();

        assert_eq!(tx, tx2);
    }

    #[test]
    fn decode_ola_raw_tx() {
        let private_key = H256::random();
        let address = PackedEthSignature::address_from_private_key(&private_key).unwrap();

        let mut tx = TransactionRequest {
            nonce: U256::from(0u32),
            to: Some(Address::random()),
            from: Some(address),
            input: Bytes::from(vec![1, 2, 3, 4, 5, 6, 7, 8]),
            transaction_type: Some(U64::from(OLA_RAW_TX_TYPE)),
            eip712_meta: Some(Eip712Meta {
                factory_deps: Some(vec![vec![2; 32]]),
                custom_signature: Some(vec![1; 32]),
                paymaster_params: Some(PaymasterParams {
                    paymaster: Default::default(),
                    paymaster_input: vec![],
                }),
            }),
            chain_id: Some(1027),
            ..Default::default()
        };

        let msg = tx.get_default_signed_message(1027);
        let signature = PackedEthSignature::sign_raw(&private_key, &msg).unwrap();

        let mut rlp = RlpStream::new();
        tx.rlp(&mut rlp, 1027, Some(&signature));
        let mut data = rlp.out().to_vec();
        data.insert(0, EIP_712_TX_TYPE);
        tx.raw = Some(Bytes(data.clone()));
        tx.v = Some(U64::from(signature.v()));
        tx.r = Some(U256::from_big_endian(signature.r()));
        tx.s = Some(U256::from_big_endian(signature.s()));
        dbg!(hex::encode(data.clone()));

        let (tx2, _) = TransactionRequest::from_bytes(&data, 1027).unwrap();

        assert_eq!(tx, tx2);
    }
}
