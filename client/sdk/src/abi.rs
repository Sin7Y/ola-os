use ola_lang_abi::{Abi, FixedArray4, Value};
use ola_types::Address;
use ola_utils::{h256_to_u64_array, u64s_to_bytes};

use crate::errors::ClientError;

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
    dbg!("{biz_calldata: }", biz_calldata.clone());
    let aa_calldata = build_aa_calldata(&to, biz_calldata)?;
    let entry_point_calldata = build_entry_point_calldata(from, to, aa_calldata, codes)?;
    dbg!("{entry_point_calldata: }", entry_point_calldata.clone());
    let calldata_bytes = u64s_to_bytes(&entry_point_calldata);
    Ok(calldata_bytes)
}

fn build_aa_calldata(to: &Address, biz_calldata: Vec<u64>) -> anyhow::Result<Vec<u64>> {
    let aa_abi_str = include_str!("abi/AAInterface.json");
    let abi: Abi = serde_json::from_str(aa_abi_str).map_err(|_| ClientError::AbiParseError)?;

    let func = abi
        .functions
        .iter()
        .find(|func| func.name == "executeTransaction".to_string())
        .expect("executeTransaction function not found");

    let params = [
        Value::Address(FixedArray4(h256_to_u64_array(to))),
        Value::Fields(biz_calldata),
    ];
    let input = abi
        .encode_input_with_signature(func.signature().as_str(), &params)
        .map_err(|_| ClientError::AbiParseError)?;
    dbg!("{aa calldata: }", input.clone());
    Ok(input)
}

fn build_entry_point_calldata(
    from: &Address,
    to: &Address,
    aa_calldata: Vec<u64>,
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
            ("data".to_string(), Value::Fields(aa_calldata)),
            (
                "codes".to_string(),
                Value::Fields(codes.unwrap_or_default()),
            ),
        ]),
        Value::Bool(false),
    ];
    let input = abi
        .encode_input_with_signature(func.signature().as_str(), &params)
        .map_err(|_| ClientError::AbiParseError)?;
    Ok(input)
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
