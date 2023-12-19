use ola_lang_abi::{FixedArray4, Value};
use ola_types::{Address, ExecuteTransactionCommon, Transaction, U256};
use ola_utils::{
    bytecode::hash_bytecode, bytes_to_be_words, bytes_to_u64s, h256_to_u256, u64s_to_bytes,
};

#[derive(Debug, Default, Clone)]
pub struct TransactionData {
    pub tx_type: u8,
    pub from: Address,
    pub to: Address,
    pub nonce: u32,
    // The reserved fields that are unique for different types of transactions.
    // E.g. nonce is currently used in all transaction, but it should not be mandatory
    // in the long run.
    pub reserved: [U256; 4],
    pub data: Vec<u8>,
    pub signature: Vec<u8>,
    // The factory deps provided with the transaction.
    // Note that *only hashes* of these bytecodes are signed by the user
    // and they are used in the ABI encoding of the struct.
    pub factory_deps: Vec<Vec<u8>>,
    pub reserved_dynamic: Vec<u8>,
}

impl TransactionData {
    pub(crate) fn abi_encode_with_custom_factory_deps(
        self,
        factory_deps_hashes: Vec<U256>,
    ) -> Vec<u8> {
        let from: [u64; 4] = bytes_to_u64s(self.from.0.to_vec()).try_into().unwrap();
        let to: [u64; 4] = bytes_to_u64s(self.to.0.to_vec()).try_into().unwrap();
        let reserved: Vec<u64> = self
            .reserved
            .iter()
            .copied()
            .flat_map(|val| val.0)
            .collect();
        let data = bytes_to_u64s(self.data);
        let signature = bytes_to_u64s(self.signature);
        let factory_deps: Vec<u64> = factory_deps_hashes
            .iter()
            .copied()
            .flat_map(|val| val.0)
            .collect();
        let reserved_dynamic = bytes_to_u64s(self.reserved_dynamic);
        let params = vec![
            Value::U32(self.tx_type as u64),
            Value::Address(FixedArray4(from)),
            Value::Address(FixedArray4(to)),
            Value::U32(self.nonce as u64),
            Value::Fields(reserved),
            Value::Fields(data),
            Value::Fields(signature),
            Value::Fields(factory_deps),
            Value::Fields(reserved_dynamic),
        ];
        let data = Value::encode(&params);
        u64s_to_bytes(&data)
    }

    pub(crate) fn abi_encode(self) -> Vec<u8> {
        let factory_deps_hashes = self
            .factory_deps
            .iter()
            .map(|dep| h256_to_u256(hash_bytecode(dep)))
            .collect();
        self.abi_encode_with_custom_factory_deps(factory_deps_hashes)
    }

    pub fn into_tokens(self) -> Vec<U256> {
        let bytes = self.abi_encode();
        assert!(bytes.len() % 32 == 0);

        bytes_to_be_words(bytes)
    }
}

impl From<Transaction> for TransactionData {
    fn from(execute_tx: Transaction) -> Self {
        match execute_tx.common_data {
            ExecuteTransactionCommon::L2(common_data) => TransactionData {
                tx_type: (common_data.transaction_type as u32) as u8,
                from: common_data.initiator_address,
                to: execute_tx.execute.contract_address,
                nonce: common_data.nonce.0,
                reserved: [U256::zero(), U256::zero(), U256::zero(), U256::zero()],
                data: execute_tx.execute.calldata,
                signature: common_data.signature,
                factory_deps: execute_tx.execute.factory_deps.unwrap_or_default(),
                reserved_dynamic: vec![],
            },
            ExecuteTransactionCommon::ProtocolUpgrade(common_data) => {
                TransactionData {
                    tx_type: common_data.tx_format() as u8,
                    from: common_data.sender,
                    to: execute_tx.execute.contract_address,
                    nonce: common_data.upgrade_id as u32,
                    reserved: [U256::zero(), U256::zero(), U256::zero(), U256::zero()],
                    data: execute_tx.execute.calldata,
                    // The signature isn't checked for L1 transactions so we don't care
                    signature: vec![],
                    factory_deps: execute_tx.execute.factory_deps.unwrap_or_default(),
                    reserved_dynamic: vec![],
                }
            }
        }
    }
}
