use ola_config::sequencer::SequencerConfig;

use crate::sequencer::{
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
    pub(super) fn new(config: SequencerConfig) -> Self {
        let sealers = Self::default_sealers();
        Self { config, sealers }
    }

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

    pub(super) fn should_seal_l1_batch(
        &self,
        l1_batch_number: u64,
        block_open_timestamp_ms: u128,
        tx_count: usize,
        block_data: &SealData,
        tx_data: &SealData,
    ) -> SealResolution {
        olaos_logs::trace!(
            "Determining seal resolution for L1 batch #{l1_batch_number} with {tx_count} transactions \
             and metrics {:?}",
            block_data.execution_metrics
        );

        let mut final_seal_resolution = SealResolution::NoSeal;
        for sealer in &self.sealers {
            let seal_resolution = sealer.should_seal(
                &self.config,
                block_open_timestamp_ms,
                tx_count,
                block_data,
                tx_data,
            );
            match &seal_resolution {
                SealResolution::IncludeAndSeal
                | SealResolution::ExcludeAndSeal
                | SealResolution::Unexecutable(_) => {
                    olaos_logs::debug!(
                        "L1 batch #{l1_batch_number} processed by `{name}` with resolution {seal_resolution:?}",
                        name = sealer.prom_criterion_name()
                    );
                }
                SealResolution::NoSeal => { /* Don't do anything */ }
            }

            final_seal_resolution = final_seal_resolution.stricter(seal_resolution);
        }
        final_seal_resolution
    }

    fn default_sealers() -> Vec<Box<dyn SealCriterion>> {
        // TODO: add more sealers
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
