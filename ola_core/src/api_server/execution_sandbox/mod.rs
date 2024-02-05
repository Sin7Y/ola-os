use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use ola_contracts::BaseSystemContracts;
use ola_dal::{SqlxError, StorageProcessor};
use ola_state::postgres::PostgresStorageCaches;
use ola_types::{api, AccountTreeId, MiniblockNumber};
use tokio::runtime::Handle;

pub mod apply;
pub mod error;
pub mod execute;
pub mod validate;
pub mod vm_metrics;

#[derive(Debug, Clone)]
pub struct VmPermit {
    /// A handle to the runtime that is used to query the VM storage.
    rt_handle: Handle,
    _permit: Arc<tokio::sync::OwnedSemaphorePermit>,
}

impl VmPermit {
    fn rt_handle(&self) -> &Handle {
        &self.rt_handle
    }
}

#[derive(Debug, Clone)]
pub struct VmConcurrencyBarrier {
    limiter: Arc<tokio::sync::Semaphore>,
    max_concurrency: usize,
}

impl VmConcurrencyBarrier {
    pub fn close(&self) {
        self.limiter.close();
    }

    pub async fn wait_until_stopped(self) {
        const POLL_INTERVAL: Duration = Duration::from_millis(50);

        assert!(
            self.limiter.is_closed(),
            "Cannot wait on non-closed VM concurrency limiter"
        );

        loop {
            let current_permits = self.limiter.available_permits();
            if current_permits == self.max_concurrency {
                return;
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }
}

#[derive(Debug)]
pub struct VmConcurrencyLimiter {
    limiter: Arc<tokio::sync::Semaphore>,
    rt_handle: Handle,
}

impl VmConcurrencyLimiter {
    pub fn new(max_concurrency: usize) -> (Self, VmConcurrencyBarrier) {
        let limiter = Arc::new(tokio::sync::Semaphore::new(max_concurrency));
        let this = Self {
            limiter: Arc::clone(&limiter),
            rt_handle: Handle::current(),
        };
        let barrier = VmConcurrencyBarrier {
            limiter,
            max_concurrency,
        };
        (this, barrier)
    }

    pub async fn acquire(&self) -> Option<VmPermit> {
        let available_permits = self.limiter.available_permits();

        let start = Instant::now();
        let permit = Arc::clone(&self.limiter).acquire_owned().await.ok()?;
        let elapsed = start.elapsed();
        // We don't want to emit too many logs.
        if elapsed > Duration::from_millis(10) {
            olaos_logs::info!(
                "Permit is obtained. Available permits: {available_permits}. Took {elapsed:?}"
            );
        }
        Some(VmPermit {
            rt_handle: self.rt_handle.clone(),
            _permit: Arc::new(permit),
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TxSharedArgs {
    pub operator_account: AccountTreeId,
    pub base_system_contracts: BaseSystemContracts,
    pub caches: PostgresStorageCaches,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BlockArgs {
    block_id: api::BlockId,
    resolved_block_number: MiniblockNumber,
    block_timestamp_s: Option<u64>,
}

impl BlockArgs {
    /// Loads block information from DB.
    pub async fn new(
        connection: &mut StorageProcessor<'_>,
        block_id: api::BlockId,
    ) -> Result<Option<Self>, SqlxError> {
        let resolved_block_number = connection
            .blocks_web3_dal()
            .resolve_block_id(block_id)
            .await?;
        let Some(resolved_block_number) = resolved_block_number else {
            return Ok(None);
        };

        let block_timestamp_s = connection
            .blocks_web3_dal()
            .get_block_timestamp(resolved_block_number)
            .await?;
        Ok(Some(Self {
            block_id,
            resolved_block_number,
            block_timestamp_s,
        }))
    }

    async fn pending(connection: &mut StorageProcessor<'_>) -> Self {
        let (block_id, resolved_block_number) = get_pending_state(connection).await;
        Self {
            block_id,
            resolved_block_number,
            block_timestamp_s: None,
        }
    }
}

async fn get_pending_state(
    connection: &mut StorageProcessor<'_>,
) -> (api::BlockId, MiniblockNumber) {
    let block_id = api::BlockId::Number(api::BlockNumber::Pending);
    let resolved_block_number = connection
        .blocks_web3_dal()
        .resolve_block_id(block_id)
        .await
        .unwrap()
        .expect("Pending block should be present");
    (block_id, resolved_block_number)
}
