use ola_basic_types::Address;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TokenInfo {
    pub l1_address: Address,
    pub l2_address: Address,
    pub metadata: TokenMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TokenMetadata {
    /// Token name (e.g. "Ethereum" or "USD Coin")
    pub name: String,
    /// Token symbol (e.g. "ETH" or "USDC")
    pub symbol: String,
    /// Token precision (e.g. 18 for "ETH" so "1.0" ETH = 10e18 as U256 number)
    pub decimals: u8,
}
