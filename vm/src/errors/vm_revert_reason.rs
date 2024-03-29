use std::convert::TryFrom;
use std::fmt::{Debug, Display};

use ola_types::U256;

use super::TxRevertReason;

#[derive(Debug, thiserror::Error)]
pub enum VmRevertReasonParsingError {
    #[error("Incorrect data offset. Data: {0:?}")]
    IncorrectDataOffset(Vec<u8>),
    #[error("Input is too short. Data: {0:?}")]
    InputIsTooShort(Vec<u8>),
    #[error("Incorrect string length. Data: {0:?}")]
    IncorrectStringLength(Vec<u8>),
}

/// Rich Revert Reasons https://github.com/0xProject/ZEIPs/issues/32
#[derive(Debug, Clone, PartialEq)]
pub enum VmRevertReason {
    General {
        msg: String,
        data: Vec<u8>,
    },
    InnerTxError,
    VmError,
    Unknown {
        function_selector: Vec<u8>,
        data: Vec<u8>,
    },
}

impl VmRevertReason {
    const GENERAL_ERROR_SELECTOR: &'static [u8] = &[0x08, 0xc3, 0x79, 0xa0];
    fn parse_general_error(original_bytes: &[u8]) -> Result<Self, VmRevertReasonParsingError> {
        let bytes = &original_bytes[4..];
        if bytes.len() < 32 {
            return Err(VmRevertReasonParsingError::InputIsTooShort(bytes.to_vec()));
        }
        let data_offset = U256::from_big_endian(&bytes[0..32]).as_usize();

        // Data offset couldn't be less than 32 because data offset size is 32 bytes
        // and data offset bytes are part of the offset. Also data offset couldn't be greater than
        // data length
        if data_offset > bytes.len() || data_offset < 32 {
            return Err(VmRevertReasonParsingError::IncorrectDataOffset(
                bytes.to_vec(),
            ));
        };

        let data = &bytes[data_offset..];

        if data.len() < 32 {
            return Err(VmRevertReasonParsingError::InputIsTooShort(bytes.to_vec()));
        };

        let string_length = U256::from_big_endian(&data[0..32]).as_usize();

        if string_length + 32 > data.len() {
            return Err(VmRevertReasonParsingError::IncorrectStringLength(
                bytes.to_vec(),
            ));
        };

        let raw_data = &data[32..32 + string_length];
        Ok(Self::General {
            msg: String::from_utf8_lossy(raw_data).to_string(),
            data: original_bytes.to_vec(),
        })
    }

    pub fn to_user_friendly_string(&self) -> String {
        match self {
            // In case of `Unknown` reason we suppress it to prevent verbose Error function_selector = 0x{}
            // message shown to user.
            VmRevertReason::Unknown { .. } => "".to_owned(),
            _ => self.to_string(),
        }
    }

    pub fn encoded_data(&self) -> Vec<u8> {
        match self {
            VmRevertReason::Unknown { data, .. } => data.clone(),
            VmRevertReason::General { data, .. } => data.clone(),
            _ => vec![],
        }
    }
}

impl TryFrom<&[u8]> for VmRevertReason {
    type Error = VmRevertReasonParsingError;

    fn try_from(bytes: &[u8]) -> Result<Self, VmRevertReasonParsingError> {
        if bytes.len() < 4 {
            // Note, that when the method reverts with no data
            // the selector is empty as well.
            // For now, we only accept errors with either no data or
            // the data with complete selectors.
            if !bytes.is_empty() {
                return Err(VmRevertReasonParsingError::IncorrectStringLength(
                    bytes.to_owned(),
                ));
            }

            let result = VmRevertReason::Unknown {
                function_selector: vec![],
                data: bytes.to_vec(),
            };

            return Ok(result);
        }

        let function_selector = &bytes[0..4];
        match function_selector {
            VmRevertReason::GENERAL_ERROR_SELECTOR => Self::parse_general_error(bytes),
            _ => {
                let result = VmRevertReason::Unknown {
                    function_selector: function_selector.to_vec(),
                    data: bytes.to_vec(),
                };
                olaos_logs::warn!("Unsupported error type: {}", result);
                Ok(result)
            }
        }
    }
}

impl Display for VmRevertReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use VmRevertReason::{General, InnerTxError, Unknown, VmError};

        match self {
            General { msg, .. } => write!(f, "{}", msg),
            VmError => write!(f, "VM Error",),
            InnerTxError => write!(f, "Bootloader-based tx failed"),
            Unknown {
                function_selector,
                data,
            } => write!(
                f,
                "Error function_selector = 0x{}, data = 0x{}",
                hex::encode(function_selector),
                hex::encode(data)
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VmRevertReasonParsingResult {
    pub revert_reason: TxRevertReason,
    pub original_data: Vec<u8>,
}

impl VmRevertReasonParsingResult {
    pub fn new(revert_reason: TxRevertReason, original_data: Vec<u8>) -> Self {
        Self {
            revert_reason,
            original_data,
        }
    }
}
