use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum SandboxExecutionError {
    #[error("Account validation failed: {0}")]
    AccountValidationFailed(String),
    #[error("Paymaster validation failed: {0}")]
    PaymasterValidationFailed(String),
    #[error("Pre-paymaster preparation failed: {0}")]
    PrePaymasterPreparationFailed(String),
    #[error("From is not an account")]
    FromIsNotAnAccount,
    #[error("Bootloader failure: {0}")]
    BootloaderFailure(String),
    #[error("Revert: {0}")]
    Revert(String, Vec<u8>),
    #[error("Failed to pay for the transaction: {0}")]
    FailedToPayForTransaction(String),
    #[error("Bootloader-based tx failed")]
    InnerTxError,
    #[error(
    "Virtual machine entered unexpected state. Please contact developers and provide transaction details \
        that caused this error. Error description: {0}"
    )]
    UnexpectedVMBehavior(String),
    #[error("Transaction is unexecutable. Reason: {0}")]
    Unexecutable(String),
}
