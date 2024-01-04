use std::fs::File;

use ola_lang_abi::{Abi, FixedArray4, Value};
use ola_types::{tx::primitives::PackedEthSignature, Address};
use ola_utils::{h256_to_u64_array, u64s_to_bytes};

use crate::errors::ClientError;

pub fn create_invoke_calldata_with_abi_file(
    abi_file: File,
    function_sig: &str,
    params: Vec<Value>,
    from: &Address,
    to: &Address,
    codes: Option<Vec<u64>>,
) -> Result<Vec<u8>, ClientError> {
    let biz_calldata = get_calldata(abi_file, function_sig, params)?;
    println!("{:?}", biz_calldata);
    build_invoke_entry_point_input(from, to, biz_calldata, codes)
}

pub fn create_invoke_calldata_with_abi(
    abi: &Abi,
    function_sig: &str,
    params: Vec<Value>,
    from: &Address,
    to: &Address,
    codes: Option<Vec<u64>>,
) -> Result<Vec<u8>, ClientError> {
    let biz_calldata = abi
        .encode_input_with_signature(function_sig, &params)
        .map_err(|_| ClientError::AbiParseError)?;
    println!("{:?}", biz_calldata);
    build_invoke_entry_point_input(from, to, biz_calldata, codes)
}

fn get_calldata(
    abi_file: File,
    function_sig: &str,
    params: Vec<Value>,
) -> Result<Vec<u64>, ClientError> {
    let abi: Abi = serde_json::from_reader(abi_file).map_err(|_| ClientError::AbiParseError)?;
    let calldata = abi
        .encode_input_with_signature(function_sig, &params)
        .map_err(|_| ClientError::AbiParseError)?;
    Ok(calldata)
}

fn build_invoke_entry_point_input(
    from: &Address,
    to: &Address,
    biz_calldata: Vec<u64>,
    codes: Option<Vec<u64>>,
) -> Result<Vec<u8>, ClientError> {
    let entry_point_abi_str = include_str!("abi/EntryPoint.json");
    let abi: Abi =
        serde_json::from_str(entry_point_abi_str).map_err(|_| ClientError::AbiParseError)?;
    let func = abi.functions[0].clone();

    // let function_sig = "system_entrance(tuple(address,address,fields,fields),bool)";

    let params = [
        Value::Tuple(vec![
            (
                "from".to_string(),
                Value::Address(FixedArray4(h256_to_u64_array(from))),
            ),
            (
                "to".to_string(),
                Value::Address(FixedArray4(h256_to_u64_array(to))),
            ),
            ("data".to_string(), Value::Fields(biz_calldata)),
            (
                "codes".to_string(),
                Value::Fields(codes.unwrap_or_default()),
            ),
        ]),
        Value::Bool(false),
    ];
    let input = abi
        .encode_input_with_signature(func.signature().as_str(), &params)
        .map(|data| {
            println!("{:?}", data.clone());
            u64s_to_bytes(&data)
        })
        .map_err(|_| ClientError::AbiParseError)?;
    Ok(input)
}

#[cfg(test)]
mod tests {
    use ola_lang_abi::Value;
    use ola_types::Address;

    use super::create_invoke_calldata_with_abi_file;
    use std::fs::File;

    #[test]
    fn test_vote_calldata() {
        let abi_file =
            File::open("examples/vote_simple_abi.json").expect("failed to open ABI file");

        let function_sig = "vote_proposal(u32)";

        let params = vec![Value::U32(1)];
        let from = Address::random();
        let to = Address::random();
        let calldata =
            create_invoke_calldata_with_abi_file(abi_file, function_sig, params, &from, &to, None)
                .unwrap();
        println!("{:?}", calldata)
    }
}
