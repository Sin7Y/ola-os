use serde::{Serialize, Deserialize};
use std::ops::{Add, Deref, DerefMut, Sub};
use std::str::FromStr;
use std::num::ParseIntError;
use std::fmt;
use std::convert::{Infallible, TryFrom, TryInto};

#[macro_use]
mod macros;

basic_type!(
    L1ChainId,
    u64
);

basic_type!(
    L2ChainId,
    u16
);