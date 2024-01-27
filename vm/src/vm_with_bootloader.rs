use ola_types::{Address, U256};

// 1G = 32M * 32 B
pub const TX_ENCODING_SPACE: u32 = 1 << 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxExecutionMode {
    VerifyExecute,
    EthCall {
        missed_storage_invocation_limit: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootloaderJobType {
    TransactionExecution,
    BlockPostprocessing,
}

#[derive(Debug, Clone, Copy)]
pub enum BlockContextMode {
    NewBlock(DerivedBlockContext, U256),
    OverrideCurrent(DerivedBlockContext),
}

impl BlockContextMode {
    pub fn inner_block_context(&self) -> DerivedBlockContext {
        match *self {
            BlockContextMode::OverrideCurrent(props) => props,
            BlockContextMode::NewBlock(props, _) => props,
        }
    }

    pub fn block_number(&self) -> u32 {
        self.inner_block_context().context.block_number
    }

    pub fn timestamp(&self) -> u64 {
        self.inner_block_context().context.block_timestamp
    }

    pub fn operator_address(&self) -> Address {
        self.inner_block_context().context.operator_address
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DerivedBlockContext {
    pub context: BlockContext,
}

#[derive(Clone, Debug, Copy)]
pub struct BlockContext {
    pub block_number: u32,
    pub block_timestamp: u64,
    pub operator_address: Address,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockProperties {
    pub default_aa_code_hash: U256,
}

impl From<BlockContext> for DerivedBlockContext {
    fn from(context: BlockContext) -> Self {
        DerivedBlockContext { context }
    }
}

// pub fn get_bootloader_memory(
//     txs: Vec<TransactionData>,
//     predefined_refunds: Vec<u32>,
//     predefined_compressed_bytecodes: Vec<Vec<CompressedBytecodeInfo>>,
//     execution_mode: TxExecutionMode,
//     block_context: BlockContextMode,
// ) -> Vec<(usize, U256)> {
//     let inner_context = block_context.inner_block_context().context;

//     let block_gas_price_per_pubdata = inner_context.block_gas_price_per_pubdata();

//     let mut memory = bootloader_initial_memory(&block_context);

//     let mut previous_compressed: usize = 0;
//     let mut already_included_txs_size = 0;
//     for (tx_index_in_block, tx) in txs.into_iter().enumerate() {
//         let compressed_bytecodes = predefined_compressed_bytecodes[tx_index_in_block].clone();

//         let mut total_compressed_len_words = 0;
//         for i in compressed_bytecodes.iter() {
//             total_compressed_len_words += i.encode_call().len() / 32;
//         }

//         let memory_for_current_tx = get_bootloader_memory_for_tx(
//             tx.clone(),
//             tx_index_in_block,
//             execution_mode,
//             already_included_txs_size,
//             predefined_refunds[tx_index_in_block],
//             block_gas_price_per_pubdata as u32,
//             previous_compressed,
//             compressed_bytecodes,
//         );

//         previous_compressed += total_compressed_len_words;

//         memory.extend(memory_for_current_tx);
//         let encoded_struct = tx.into_tokens();
//         let encoding_length = encoded_struct.len();
//         already_included_txs_size += encoding_length;
//     }
//     memory
// }
