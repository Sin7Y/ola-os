use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::L1ChainId;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Network {
    Mainnet,
    Rinkeby,
    Ropsten,
    Goerli,
    Localhost,
    Unknown,
}

impl FromStr for Network {
    type Err = String;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        Ok(match string {
            "mainnet" => Self::Mainnet,
            "rinkeby" => Self::Rinkeby,
            "ropsten" => Self::Ropsten,
            "goerli" => Self::Goerli,
            "localhost" => Self::Localhost,
            other => return Err(other.to_owned()),
        })
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mainnet => write!(f, "mainnet"),
            Self::Rinkeby => write!(f, "rinkeby"),
            Self::Ropsten => write!(f, "ropsten"),
            Self::Goerli => write!(f, "goerli"),
            Self::Localhost => write!(f, "localhost"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

impl Network {
    pub fn from_chain_id(chain_id: L1ChainId) -> Self {
        match *chain_id {
            1 => Self::Mainnet,
            3 => Self::Ropsten,
            4 => Self::Rinkeby,
            5 => Self::Goerli,
            9 => Self::Localhost,
            _ => Self::Unknown,
        }
    }

    pub fn chain_id(self) -> L1ChainId {
        match self {
            Self::Mainnet => L1ChainId(1),
            Self::Ropsten => L1ChainId(3),
            Self::Rinkeby => L1ChainId(4),
            Self::Goerli => L1ChainId(5),
            Self::Localhost => L1ChainId(9),
            Self::Unknown => panic!("Unknown chain ID"),
        }
    }
}
