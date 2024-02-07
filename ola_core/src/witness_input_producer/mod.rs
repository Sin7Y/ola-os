use std::{sync::Arc, time::Instant};

use anyhow::Ok;
use async_trait::async_trait;
use ola_dal::connection::ConnectionPool;
use ola_types::{witness_block_state::WitnessBlockState, L1BatchNumber, L2ChainId};
use olaos_object_store::{ObjectStore, ObjectStoreFactory};
use olaos_queued_job_processor::JobProcessor;
use tokio::{runtime::Handle, task::JoinHandle};

#[derive(Debug)]
pub struct WitnessInputProducer {
    connection_pool: ConnectionPool,
    l2_chain_id: L2ChainId,
    object_store: Arc<dyn ObjectStore>,
}

impl WitnessInputProducer {
    pub async fn new(
        connection_pool: ConnectionPool,
        store_factory: &ObjectStoreFactory,
        l2_chain_id: L2ChainId,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            connection_pool,
            object_store: store_factory.create_store().await,
            l2_chain_id,
        })
    }

    fn process_job_impl(
        _rt_handle: Handle,
        _l1_batch_number: L1BatchNumber,
        _started_at: Instant,
        _connection_pool: ConnectionPool,
        _l2_chain_id: L2ChainId,
    ) -> anyhow::Result<WitnessBlockState> {
        // TODO:
        Ok(WitnessBlockState::default())
    }
}

#[async_trait]
impl JobProcessor for WitnessInputProducer {
    type Job = L1BatchNumber;
    type JobId = L1BatchNumber;
    type JobArtifacts = WitnessBlockState;
    const SERVICE_NAME: &'static str = "witness_input_producer";

    async fn get_next_job(&self) -> anyhow::Result<Option<(Self::JobId, Self::Job)>> {
        let _connection = self.connection_pool.access_storage().await;
        // TODO:
        Ok(Some((L1BatchNumber(0), L1BatchNumber(0))))
    }

    async fn save_failure(&self, _job_id: Self::JobId, _started_at: Instant, _error: String) {
        todo!()
    }

    async fn process_job(
        &self,
        job: Self::Job,
        started_at: Instant,
    ) -> JoinHandle<anyhow::Result<Self::JobArtifacts>> {
        let l2_chain_id = self.l2_chain_id;
        let connection_pool = self.connection_pool.clone();
        tokio::task::spawn_blocking(move || {
            let rt_handle = Handle::current();
            Self::process_job_impl(
                rt_handle,
                job,
                started_at,
                connection_pool.clone(),
                l2_chain_id,
            )
        })
    }

    async fn save_result(
        &self,
        _job_id: Self::JobId,
        _started_at: Instant,
        _artifacts: Self::JobArtifacts,
    ) -> anyhow::Result<()> {
        // TODO:
        Ok(())
    }

    fn max_attempts(&self) -> u32 {
        // TODO:
        0 as u32
    }

    async fn get_job_attempts(&self, _job_id: &L1BatchNumber) -> anyhow::Result<u32> {
        // TODO:
        Ok(0)
    }
}
