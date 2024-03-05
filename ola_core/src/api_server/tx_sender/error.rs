use ola_types::l2::error::TxCheckError;

use ola_web3_decl::error::EnrichedClientError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubmitTxError {
    #[error("nonce too high. allowed nonce range: {0} - {1}, actual: {2}")]
    NonceIsTooHigh(u32, u32, u32),
    #[error("nonce too low. allowed nonce range: {0} - {1}, actual: {2}")]
    NonceIsTooLow(u32, u32, u32),
    #[error("{0}")]
    IncorrectTx(#[from] TxCheckError),
    #[error("execution reverted{}{}" , if .0.is_empty() { "" } else { ": " }, .0)]
    PreExecutionReverted(String, Vec<u8>),
    #[error("execution reverted{}{}" , if .0.is_empty() { "" } else { ": " }, .0)]
    ExecutionReverted(String, Vec<u8>),
    #[error("{0}")]
    Unexecutable(String),
    #[error("too many transactions")]
    RateLimitExceeded,
    #[error("server shutting down")]
    ServerShuttingDown,
    #[error("failed to include transaction in the system. reason: {0}")]
    BootloaderFailure(String),
    #[error("failed to validate the transaction. reason: {0}")]
    ValidationFailed(String),
    #[error("not enough balance to cover the fee. error message: {0}")]
    FailedToChargeFee(String),
    #[error("failed paymaster validation. error message: {0}")]
    PaymasterValidationFailed(String),
    #[error("failed pre-paymaster preparation. error message: {0}")]
    PrePaymasterPreparationFailed(String),
    #[error("invalid sender. can't start a transaction from a non-account")]
    FromIsNotAnAccount,
    #[error(
        "virtual machine entered unexpected state. please contact developers and provide transaction details \
        that caused this error. Error description: {0}"
    )]
    UnexpectedVMBehavior(String),
    #[error("pubdata price limit is too low, ensure that the price limit is correct")]
    UnrealisticPubdataPriceLimit,
    #[error(
        "too many factory dependencies in the transaction. {0} provided, while only {1} allowed"
    )]
    TooManyFactoryDependencies(usize, usize),
    #[error("max fee per pubdata byte higher than 2^32")]
    FeePerPubdataByteTooHigh,
    /// InsufficientFundsForTransfer is returned if the transaction sender doesn't
    /// have enough funds for transfer.
    #[error("insufficient balance for transfer")]
    InsufficientFundsForTransfer,
    /// Error returned from main node
    #[error("{0}")]
    ProxyError(#[from] EnrichedClientError),
    #[error("tx call vm failed: {0}")]
    TxCallTxError(String),
}

impl SubmitTxError {
    pub fn data(&self) -> Vec<u8> {
        if let Self::ExecutionReverted(_, data) = self {
            data.clone()
        } else {
            Vec::new()
        }
    }
}
