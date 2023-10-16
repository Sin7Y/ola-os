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
