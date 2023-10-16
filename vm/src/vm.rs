use ola_types::U256;

use crate::Word;

#[derive(Debug, PartialEq, Default)]
pub struct VmExecutionResult {
    pub used_contract_hashes: Vec<U256>,
    pub return_data: Vec<Word>,
    pub contracts_used: usize,
    pub cycles_used: u32,
}
