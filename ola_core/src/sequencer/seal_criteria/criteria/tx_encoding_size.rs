use ola_config::sequencer::SequencerConfig;
use ola_vm::vm_with_bootloader::TX_ENCODING_SPACE;

use crate::sequencer::{
    seal_criteria::{SealCriterion, SealResolution},
    SealData,
};

#[derive(Debug)]
pub struct TxEncodingSizeCriterion;

impl SealCriterion for TxEncodingSizeCriterion {
    fn should_seal(
        &self,
        config: &SequencerConfig,
        _block_open_timestamp_ms: u128,
        _tx_count: usize,
        block_data: &SealData,
        tx_data: &SealData,
    ) -> SealResolution {
        let reject_bound =
            (TX_ENCODING_SPACE as f64 * config.reject_tx_at_geometry_percentage).round();
        let include_and_seal_bound = (TX_ENCODING_SPACE as f64
            * config.close_block_at_geometry_percentage)
            .round();

        if tx_data.cumulative_size > reject_bound as usize {
            let message = "Transaction cannot be included due to large encoding size";
            SealResolution::Unexecutable(message.into())
        } else if block_data.cumulative_size > TX_ENCODING_SPACE as usize {
            SealResolution::ExcludeAndSeal
        } else if block_data.cumulative_size > include_and_seal_bound as usize {
            SealResolution::IncludeAndSeal
        } else {
            SealResolution::NoSeal
        }
    }

    fn prom_criterion_name(&self) -> &'static str {
        "tx_encoding_size"
    }
}
