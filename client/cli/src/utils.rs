use anyhow::Result;
use ethereum_types::H256;

pub(crate) fn from_hex_be(value: &str) -> Result<H256> {
    let value = value.trim_start_matches("0x");

    let hex_chars_len = value.len();
    let expected_hex_length = 64;

    let parsed_bytes: [u8; 32] = if hex_chars_len == expected_hex_length {
        let mut buffer = [0u8; 32];
        hex::decode_to_slice(value, &mut buffer)?;
        buffer
    } else if hex_chars_len < expected_hex_length {
        let mut padded_hex = str::repeat("0", expected_hex_length - hex_chars_len);
        padded_hex.push_str(value);

        let mut buffer = [0u8; 32];
        hex::decode_to_slice(&padded_hex, &mut buffer)?;
        buffer
    } else {
        anyhow::bail!("Key out of range.");
    };
    Ok(H256(parsed_bytes))
}
