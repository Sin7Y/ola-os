use std::{fmt, marker::PhantomData};

use itertools::unfold;
use ola_types::{api::Log, L1BatchNumber, H256};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PubSubResult {
    Log(Log),
    TxHash(H256),
    Syncing(bool),
    L1BatchProof(L1BatchProofForVerify),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1BatchProofForVerify {
    pub l1_batch_number: L1BatchNumber,
    pub prove_batches_data: Vec<u8>,
}

/// Either value or array of values.
///
/// A value must serialize into a string.
#[derive(Default, Debug, PartialEq, Clone)]
pub struct ValueOrArray<T>(pub Vec<T>);

impl<T> From<T> for ValueOrArray<T> {
    fn from(value: T) -> Self {
        Self(vec![value])
    }
}

impl<T: Serialize> Serialize for ValueOrArray<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0.len() {
            0 => serializer.serialize_none(),
            1 => Serialize::serialize(&self.0[0], serializer),
            _ => Serialize::serialize(&self.0, serializer),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for ValueOrArray<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> de::Visitor<'de> for Visitor<T> {
            type Value = ValueOrArray<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("string value or sequence of values")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                use serde::de::IntoDeserializer;

                Deserialize::deserialize(value.into_deserializer())
                    .map(|value| ValueOrArray(vec![value]))
            }

            fn visit_seq<S>(self, visitor: S) -> Result<Self::Value, S::Error>
            where
                S: de::SeqAccess<'de>,
            {
                unfold(visitor, |vis| vis.next_element().transpose())
                    .collect::<Result<_, _>>()
                    .map(ValueOrArray)
            }
        }

        deserializer.deserialize_any(Visitor(PhantomData))
    }
}

#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct PubSubFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<ValueOrArray<H256>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<Option<ValueOrArray<H256>>>>,
}

impl PubSubFilter {
    pub fn matches(&self, log: &Log) -> bool {
        if let Some(addresses) = &self.address {
            if !addresses.0.contains(&log.address) {
                return false;
            }
        }
        if let Some(all_topics) = &self.topics {
            for (idx, expected_topics) in all_topics.iter().enumerate() {
                if let Some(expected_topics) = expected_topics {
                    if let Some(actual_topic) = log.topics.get(idx) {
                        if !expected_topics.0.contains(actual_topic) {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }
        }
        true
    }
}
