use std::fmt;

use serde::{
    de::{Error, Unexpected, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Bytes8(pub Vec<u64>);

impl<T: Into<Vec<u64>>> From<T> for Bytes8 {
    fn from(data: T) -> Self {
        Bytes8(data.into())
    }
}

impl Serialize for Bytes8 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serialized = "0x".to_owned();
        let data: Vec<u8> = self.0.iter().flat_map(|w| w.to_be_bytes()).collect();
        serialized.push_str(&hex::encode(&data));
        serializer.serialize_str(serialized.as_ref())
    }
}

impl<'a> Deserialize<'a> for Bytes8 {
    fn deserialize<D>(deserializer: D) -> Result<Bytes8, D::Error>
    where
        D: Deserializer<'a>,
    {
        deserializer.deserialize_identifier(Bytes8Visitor)
    }
}

impl fmt::Debug for Bytes8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data: Vec<u8> = self.0.iter().flat_map(|w| w.to_be_bytes()).collect();
        let serialized = format!("0x{}", hex::encode(&data));
        f.debug_tuple("Bytes").field(&serialized).finish()
    }
}

struct Bytes8Visitor;

impl<'a> Visitor<'a> for Bytes8Visitor {
    type Value = Bytes8;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "a 0x-prefixed hex-encoded vector of bytes8(u64 vector)"
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        if let Some(value) = value.strip_prefix("0x") {
            let bytes =
                hex::decode(value).map_err(|e| Error::custom(format!("Invalid hex: {}", e)))?;
            let bytes8 = bytes
                .chunks(8)
                .map(|chunk| u64::from_be_bytes(chunk.try_into().unwrap()))
                .collect();
            Ok(Bytes8(bytes8))
        } else {
            Err(Error::invalid_value(Unexpected::Str(value), &"0x prefix"))
        }
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_str(value.as_ref())
    }
}
