use ethereum_types::{H256, H512};
use ola_lang_abi::{Abi, FixedArray4, Value};
use ola_types::{l2::L2Tx, request::CallRequest, request::PaymasterParams, Address, Bytes, Nonce};
use ola_utils::{h256_to_string, h256_to_u64_array, u64s_to_bytes};

use crate::{errors::ClientError, utils::h512_to_u64_array};

pub fn create_set_public_key_calldata(from: &Address, pub_key: H512) -> anyhow::Result<Vec<u8>> {
    let abi_str = include_str!("abi/DefaultAccountAbi.json");
    let abi: Abi = serde_json::from_str(abi_str).map_err(|_| ClientError::AbiParseError)?;
    let pub_key_fes = h512_to_u64_array(pub_key)?;
    let params = vec![Value::Fields(pub_key_fes.to_vec())];
    let to = H256([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x80, 0x06,
    ]);
    let func = abi
        .functions
        .iter()
        .find(|func| func.name == "setPubkey".to_string())
        .expect("function not found");
    create_calldata(&abi, func.signature().as_str(), params, from, &to, None)
}

pub fn create_calldata(
    abi: &Abi,
    function_sig: &str,
    params: Vec<Value>,
    from: &Address,
    to: &Address,
    codes: Option<Vec<u64>>,
) -> anyhow::Result<Vec<u8>> {
    let biz_calldata = abi
        .encode_input_with_signature(function_sig, &params)
        .map_err(|_| ClientError::AbiParseError)?;
    // dbg!(biz_calldata.clone());
    let entry_point_calldata = build_entry_point_calldata(from, to, biz_calldata, codes)?;
    // dbg!(entry_point_calldata.clone());
    let calldata_bytes = u64s_to_bytes(&entry_point_calldata);
    Ok(calldata_bytes)
}

fn build_entry_point_calldata(
    from: &Address,
    to: &Address,
    biz_calldata: Vec<u64>,
    codes: Option<Vec<u64>>,
) -> anyhow::Result<Vec<u64>> {
    let entry_point_abi_str = include_str!("abi/EntryPointAbi.json");
    let abi: Abi =
        serde_json::from_str(entry_point_abi_str).map_err(|_| ClientError::AbiParseError)?;

    let func = abi
        .functions
        .iter()
        .find(|func| func.name == "system_entrance".to_string())
        .expect("system_entrance function not found");

    let code_value = match codes {
        Some(codes) => Value::Fields(codes),
        None => Value::Fields(vec![]),
    };

    let params = [
        Value::Tuple(vec![
            (
                "from".to_string(),
                Value::Address(FixedArray4::from(h256_to_string(from).as_str())),
            ),
            (
                "to".to_string(),
                Value::Address(FixedArray4::from(h256_to_string(to).as_str())),
            ),
            ("data".to_string(), Value::Fields(biz_calldata)),
            ("codes".to_string(), code_value),
        ]),
        Value::Bool(false),
    ];
    let input = abi
        .encode_input_with_signature(func.signature().as_str(), &params)
        .map_err(|_| ClientError::AbiParseError)?;
    Ok(input)
}

pub fn build_call_request(
    abi: &Abi,
    function_sig: &str,
    params: Vec<Value>,
    from: &Address,
    to: &Address,
) -> anyhow::Result<CallRequest> {
    let biz_calldata = abi
        .encode_input_with_signature(function_sig, &params)
        .map_err(|_| ClientError::AbiParseError)?;

    let calldata_bytes = u64s_to_bytes(&biz_calldata);

    Ok(CallRequest::builder()
        .from(from.clone())
        .to(to.clone())
        .data(Bytes(calldata_bytes))
        .build())
}

// #[cfg(test)]
// mod tests {
//     use ola_lang_abi::Value;
//     use ola_types::Address;

//     use super::create_invoke_calldata_with_abi_file;
//     use std::fs::File;

//     #[test]
//     fn test_vote_calldata() {
//         let abi_file =
//             File::open("examples/vote_simple_abi.json").expect("failed to open ABI file");

//         let function_sig = "vote_proposal(u32)";

//         let params = vec![Value::U32(1)];
//         let from = Address::random();
//         let to = Address::random();
//         let calldata =
//             create_invoke_calldata_with_abi_file(abi_file, function_sig, params, &from, &to, None)
//                 .unwrap();
//         println!("{:?}", calldata)
//     }
// }
