use ola_config::sequencer::SequencerConfig;

use crate::api_server::sequencer::{
    seal_criteria::{criteria, SealResolution},
    SealData,
};

use super::SealCriterion;

#[derive(Debug)]
pub struct ConditionalSealer {
    config: SequencerConfig,
    /// Primary sealers set that is used to check if batch should be sealed after executing a transaction.
    sealers: Vec<Box<dyn SealCriterion>>,
}

impl ConditionalSealer {
    /// Finds a reason why a transaction with the specified `data` is unexecutable.
    pub(crate) fn find_unexecutable_reason(
        config: &SequencerConfig,
        data: &SealData,
    ) -> Option<&'static str> {
        for sealer in &Self::default_sealers() {
            const MOCK_BLOCK_TIMESTAMP: u128 = 0;
            const TX_COUNT: usize = 1;

            let resolution = sealer.should_seal(config, MOCK_BLOCK_TIMESTAMP, TX_COUNT, data, data);
            if matches!(resolution, SealResolution::Unexecutable(_)) {
                return Some(sealer.prom_criterion_name());
            }
        }
        None
    }

    fn default_sealers() -> Vec<Box<dyn SealCriterion>> {
        vec![
            Box::new(criteria::SlotsCriterion),
            // Box::new(criteria::PubDataBytesCriterion),
            // Box::new(criteria::InitialWritesCriterion),
            // Box::new(criteria::RepeatedWritesCriterion),
            // Box::new(criteria::MaxCyclesCriterion),
            // Box::new(criteria::TxEncodingSizeCriterion),
        ]
    }
}
