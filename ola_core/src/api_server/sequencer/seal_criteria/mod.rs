use std::fmt;

use ola_config::sequencer::SequencerConfig;

use super::SealData;

pub mod conditional_sealer;
pub mod criteria;

pub(super) trait SealCriterion: fmt::Debug + Send + 'static {
    fn should_seal(
        &self,
        config: &SequencerConfig,
        block_open_timestamp_ms: u128,
        tx_count: usize,
        block_data: &SealData,
        tx_data: &SealData,
    ) -> SealResolution;

    // We need self here only for rust restrictions for creating an object from trait
    // https://doc.rust-lang.org/reference/items/traits.html#object-safety
    fn prom_criterion_name(&self) -> &'static str;
}

#[derive(Debug, Clone, PartialEq)]
pub enum SealResolution {
    /// Block should not be sealed right now.
    NoSeal,
    /// Latest transaction should be included into the block and sealed after that.
    IncludeAndSeal,
    /// Latest transaction should be excluded from the block and become the first
    /// tx in the next block.
    /// While it may be kinda counter-intuitive that we first execute transaction and just then
    /// decided whether we should include it into the block or not, it is required by the architecture of
    /// zkSync Era. We may not know, for example, how much gas block will consume, because 1) smart contract
    /// execution is hard to predict and 2) we may have writes to the same storage slots, which will save us
    /// gas.
    ExcludeAndSeal,
    /// Unexecutable means that the last transaction of the block cannot be executed even
    /// if the block will consist of it solely. Such a transaction must be rejected.
    ///
    /// Contains a reason for why transaction was considered unexecutable.
    Unexecutable(String),
}
