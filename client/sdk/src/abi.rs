use std::fs::File;

use ola_lang_abi::{Abi, Value};
use ola_utils::u64s_to_bytes;

use crate::errors::ClientError;

pub fn get_calldata(
    abi_file: File,
    function_sig: &str,
    params: Vec<Value>,
) -> Result<Vec<u8>, ClientError> {
    let abi: Abi = serde_json::from_reader(abi_file).map_err(|_| ClientError::AbiParseError)?;
    let calldata = abi
        .encode_input_with_signature(function_sig, &params)
        .map(|data| u64s_to_bytes(&data))
        .map_err(|_| ClientError::AbiParseError)?;
    Ok(calldata)
}
