use ola_config::sequencer::SequencerConfig;

use crate::api_server::sequencer::{
    seal_criteria::{SealCriterion, SealResolution},
    SealData,
};

#[derive(Debug)]
pub struct SlotsCriterion;

impl SealCriterion for SlotsCriterion {
    fn should_seal(
        &self,
        config: &SequencerConfig,
        _block_open_timestamp_ms: u128,
        tx_count: usize,
        _block_data: &SealData,
        _tx_data: &SealData,
    ) -> SealResolution {
        if tx_count >= config.transaction_slots {
            SealResolution::IncludeAndSeal
        } else {
            SealResolution::NoSeal
        }
    }

    fn prom_criterion_name(&self) -> &'static str {
        "slots"
    }
}
