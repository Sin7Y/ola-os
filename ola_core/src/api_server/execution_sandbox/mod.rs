use std::{sync::Arc, time::Duration};

use tokio::runtime::Handle;

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
}
