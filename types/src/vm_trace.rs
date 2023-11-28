use std::fmt;

use ola_basic_types::{bytes8::Bytes8, Address};
use ola_config::constants::contracts::BOOTLOADER_ADDRESS;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FarCallOpcode {
    Normal = 0,
    Delegate,
    Mimic,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum CallType {
    #[serde(serialize_with = "far_call_type_to_u8")]
    #[serde(deserialize_with = "far_call_type_from_u8")]
    Call(FarCallOpcode),
    Create,
    NearCall,
}

// TODO: @Pierre
#[derive(Clone, Serialize, Deserialize)]
/// Represents a call in the VM trace.
pub struct Call {
    /// Type of the call.
    pub r#type: CallType,
    /// Address of the caller.
    pub from: Address,
    /// Address of the callee.
    pub to: Address,
    /// Input data.
    pub input: Bytes8,
    /// Output data.
    pub output: Bytes8,
    /// Error message provided by vm or some unexpected errors.
    pub error: Option<String>,
    /// Revert reason.
    pub revert_reason: Option<String>,
    /// Subcalls.
    pub calls: Vec<Call>,
}

impl PartialEq for Call {
    fn eq(&self, other: &Self) -> bool {
        self.revert_reason == other.revert_reason
            && self.input == other.input
            && self.from == other.from
            && self.to == other.to
            && self.r#type == other.r#type
            && self.error == other.error
            && self.output == other.output
            && self.calls == other.calls
    }
}

impl Default for Call {
    fn default() -> Self {
        Self {
            r#type: CallType::Call(FarCallOpcode::Normal),
            from: Default::default(),
            to: Default::default(),
            input: Bytes8(vec![]),
            output: Bytes8(vec![]),
            error: None,
            revert_reason: None,
            calls: vec![],
        }
    }
}

impl fmt::Debug for Call {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Call")
            .field("type", &self.r#type)
            .field("to", &self.to)
            .field("from", &self.from)
            .field("input", &format_args!("{:?}", self.input))
            .field("output", &format_args!("{:?}", self.output))
            .field("error", &self.error)
            .field("revert_reason", &format_args!("{:?}", self.revert_reason))
            .field("call_traces", &self.calls)
            .finish()
    }
}

impl Call {
    pub fn new_high_level(
        input: Bytes8,
        output: Bytes8,
        revert_reason: Option<String>,
        calls: Vec<Call>,
    ) -> Self {
        Self {
            r#type: CallType::Call(FarCallOpcode::Normal),
            from: Address::zero(),
            to: BOOTLOADER_ADDRESS,
            input,
            output,
            error: None,
            revert_reason,
            calls,
        }
    }
}

fn far_call_type_from_u8<'de, D>(deserializer: D) -> Result<FarCallOpcode, D::Error>
where
    D: Deserializer<'de>,
{
    let res = u8::deserialize(deserializer)?;
    match res {
        0 => Ok(FarCallOpcode::Normal),
        1 => Ok(FarCallOpcode::Delegate),
        2 => Ok(FarCallOpcode::Mimic),
        _ => Err(serde::de::Error::custom("Invalid FarCallOpcode")),
    }
}

fn far_call_type_to_u8<S>(far_call_type: &FarCallOpcode, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_u8(*far_call_type as u8)
}
