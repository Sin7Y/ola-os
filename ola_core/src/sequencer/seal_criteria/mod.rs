use std::fmt;

use ola_config::sequencer::SequencerConfig;

use self::conditional_sealer::ConditionalSealer;

use super::{extractors, SealData};

use ola_utils::time::millis_since;

use super::updates::UpdatesManager;

pub mod conditional_sealer;
pub mod criteria;

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

impl SealResolution {
    /// Compares two seal resolutions and chooses the one that is stricter.
    /// `Unexecutable` is stricter than `ExcludeAndSeal`.
    /// `ExcludeAndSeal` is stricter than `IncludeAndSeal`.
    /// `IncludeAndSeal` is stricter than `NoSeal`.
    pub fn stricter(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unexecutable(reason), _) | (_, Self::Unexecutable(reason)) => {
                Self::Unexecutable(reason)
            }
            (Self::ExcludeAndSeal, _) | (_, Self::ExcludeAndSeal) => Self::ExcludeAndSeal,
            (Self::IncludeAndSeal, _) | (_, Self::IncludeAndSeal) => Self::IncludeAndSeal,
            _ => Self::NoSeal,
        }
    }

    /// Returns `true` if L1 batch should be sealed according to this resolution.
    pub fn should_seal(&self) -> bool {
        matches!(self, Self::IncludeAndSeal | Self::ExcludeAndSeal)
    }

    /// Name of this resolution usable as a metric label.
    pub fn name(&self) -> &'static str {
        match self {
            Self::NoSeal => "no_seal",
            Self::IncludeAndSeal => "include_and_seal",
            Self::ExcludeAndSeal => "exclude_and_seal",
            Self::Unexecutable(_) => "unexecutable",
        }
    }
}

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

pub type SealerFn = dyn Fn(&UpdatesManager) -> bool + Send;

pub struct SealManager {
    /// Conditional sealer, i.e. one that can decide whether the batch should be sealed after executing a tx.
    /// Currently, it's expected to be `Some` on the main node and `None` on the external nodes, since external nodes
    /// do not decide whether to seal the batch or not.
    conditional_sealer: Option<ConditionalSealer>,
    /// Unconditional batch sealer, i.e. one that can be used if we should seal the batch *without* executing a tx.
    /// If any of the unconditional sealers returns `true`, the batch will be sealed.
    ///
    /// Note: only non-empty batch can be sealed.
    unconditional_sealers: Vec<Box<SealerFn>>,
    /// Miniblock sealer function used to determine if we should seal the miniblock.
    /// If any of the miniblock sealers returns `true`, the miniblock will be sealed.
    miniblock_sealers: Vec<Box<SealerFn>>,
}

impl fmt::Debug for SealManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SealManager")
            .finish_non_exhaustive()
    }
}

impl SealManager {
    /// Creates a default pre-configured seal manager for the main node.
    pub(super) fn new(config: SequencerConfig) -> Self {
        let timeout_batch_sealer = Self::timeout_batch_sealer(config.block_commit_deadline_ms);
        let timeout_miniblock_sealer =
            Self::timeout_miniblock_sealer(config.miniblock_commit_deadline_ms);
        // Currently, it's assumed that timeout is the only criterion for miniblock sealing.
        // If this doesn't hold and some miniblocks are sealed in less than 1 second,
        // then sequencer will be blocked waiting for the miniblock timestamp to be changed.
        let miniblock_sealers = vec![timeout_miniblock_sealer];

        let conditional_sealer = ConditionalSealer::new(config);

        Self::custom(
            Some(conditional_sealer),
            vec![timeout_batch_sealer],
            miniblock_sealers,
        )
    }

    pub fn custom(
        conditional_sealer: Option<ConditionalSealer>,
        unconditional_sealers: Vec<Box<SealerFn>>,
        miniblock_sealers: Vec<Box<SealerFn>>,
    ) -> Self {
        Self {
            conditional_sealer,
            unconditional_sealers,
            miniblock_sealers,
        }
    }

    fn timeout_batch_sealer(block_commit_deadline_ms: u64) -> Box<SealerFn> {
        const RULE_NAME: &str = "no_txs_timeout";

        Box::new(move |manager| {
            // Verify timestamp
            let should_seal_timeout =
                millis_since(manager.batch_timestamp()) > block_commit_deadline_ms;

            if should_seal_timeout {
                olaos_logs::debug!(
                    "Decided to seal L1 batch using rule `{RULE_NAME}`; batch timestamp: {}, \
                     commit deadline: {block_commit_deadline_ms}ms",
                    extractors::display_timestamp(manager.batch_timestamp())
                );
            }
            should_seal_timeout
        })
    }

    fn timeout_miniblock_sealer(miniblock_commit_deadline_ms: u64) -> Box<SealerFn> {
        if miniblock_commit_deadline_ms < 1000 {
            panic!("`miniblock_commit_deadline_ms` should be at least 1000, because miniblocks must have different timestamps");
        }

        Box::new(move |manager| {
            !manager.miniblock.executed_transactions.is_empty()
                && millis_since(manager.miniblock.timestamp) > miniblock_commit_deadline_ms
        })
    }

    pub(super) fn should_seal_l1_batch_unconditionally(
        &self,
        updates_manager: &UpdatesManager,
    ) -> bool {
        // Regardless of which sealers are provided, we never want to seal an empty batch.
        updates_manager.pending_executed_transactions_len() != 0
            && self
                .unconditional_sealers
                .iter()
                .any(|sealer| (sealer)(updates_manager))
    }

    pub(super) fn should_seal_miniblock(&self, updates_manager: &UpdatesManager) -> bool {
        // Unlike with the L1 batch, we don't check the number of transactions in the miniblock,
        // because we might want to seal the miniblock even if it's empty (e.g. on an external node,
        // where we have to replicate the state of the main node, including the last (empty) miniblock of the batch).
        // The check for the number of transactions is expected to be done, if relevant, in the `miniblock_sealer`
        // directly.
        self.miniblock_sealers
            .iter()
            .any(|sealer| (sealer)(updates_manager))
    }

    pub(super) fn should_seal_l1_batch(
        &self,
        l1_batch_number: u64,
        block_open_timestamp_ms: u128,
        tx_count: usize,
        block_data: &SealData,
        tx_data: &SealData,
    ) -> SealResolution {
        if let Some(sealer) = &self.conditional_sealer {
            sealer.should_seal_l1_batch(
                l1_batch_number,
                block_open_timestamp_ms,
                tx_count,
                block_data,
                tx_data,
            )
        } else {
            SealResolution::NoSeal
        }
    }
}
