use std::time::Duration;

use anyhow::Ok;
use ola_config::chain::MempoolConfig;
use ola_dal::connection::ConnectionPool;
use tokio::sync::watch;

use super::types::MempoolGuard;

#[derive(Debug)]
pub struct MempoolFetcher {
    mempool: MempoolGuard,
    sync_interval: Duration,
    sync_batch_size: usize,
}

impl MempoolFetcher {
    pub fn new(mempool: MempoolGuard, config: &MempoolConfig) -> Self {
        Self {
            mempool,
            sync_interval: config.sync_interval(),
            sync_batch_size: config.sync_batch_size,
        }
    }

    pub async fn run(
        mut self,
        pool: ConnectionPool,
        remove_stuck_txs: bool,
        stuck_tx_timeout: Duration,
        stop_receiver: watch::Receiver<bool>,
    ) -> anyhow::Result<()> {
        {
            let mut storage = pool.access_storage_tagged("sequencer").await;
            if remove_stuck_txs {
                let removed_txs = storage
                    .transactions_dal()
                    .remove_stuck_txs(stuck_tx_timeout)
                    .await;
                olaos_logs::info!("Number of stuck txs was removed: {}", removed_txs);
            }
            storage.transactions_dal().reset_mempool().await;
        }

        loop {
            if *stop_receiver.borrow() {
                olaos_logs::info!("Stop signal received, mempool is shutting down");
                break;
            }
            let mut storage = pool.access_storage_tagged("sequencer").await;
            let mempool_info = self.mempool.get_mempool_info();

            if mempool_info.stashed_accounts.len() > 0 {
                olaos_logs::info!("stashed accounts {:?}", mempool_info.stashed_accounts);
            }

            if mempool_info.purged_accounts.len() > 0 {
                olaos_logs::info!("purged accounts {:?}", mempool_info.purged_accounts);
            }

            let (transactions, nonces) = storage
                .transactions_dal()
                .sync_mempool(
                    mempool_info.stashed_accounts,
                    mempool_info.purged_accounts,
                    self.sync_batch_size,
                )
                .await;
            if transactions.len() > 0 {
                olaos_logs::info!("Sync {:?} transactions from mempool", transactions.len());
                for tx in transactions.iter() {
                    olaos_logs::info!(
                        "tx hash {:?}, initiator_address {:?}, tx nonce {:?}",
                        tx.hash(),
                        tx.initiator_account(),
                        tx.nonce()
                    );
                }
                for (addr, nonce) in nonces.iter() {
                    olaos_logs::info!("account address {:?}, nonce {:?}", addr, nonce,);
                }
            }

            let all_transactions_loaded = transactions.len() < self.sync_batch_size;
            self.mempool.insert(transactions, nonces);
            if all_transactions_loaded {
                tokio::time::sleep(self.sync_interval).await;
            }
        }
        Ok(())
    }
}
