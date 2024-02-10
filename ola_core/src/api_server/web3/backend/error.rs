use ola_web3_decl::error::Web3Error;

pub fn internal_error(method_name: &str, error: impl ToString) -> Web3Error {
    olaos_logs::error!(
        "Internal error in method {}: {}",
        method_name,
        error.to_string(),
    );

    Web3Error::InternalError
}
