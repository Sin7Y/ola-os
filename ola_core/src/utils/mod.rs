use std::time::Duration;

use ola_dal::connection::ConnectionPool;
use ola_types::L1BatchNumber;
use tokio::sync::watch;

/// Repeatedly polls the DB until there is an L1 batch. We may not have such a batch initially
/// if the DB is recovered from an application-level snapshot.
///
/// Returns the number of the *earliest* L1 batch, or `None` if the stop signal is received.
pub(crate) async fn wait_for_l1_batch(
    pool: &ConnectionPool,
    poll_interval: Duration,
    stop_receiver: &mut watch::Receiver<bool>,
) -> anyhow::Result<Option<L1BatchNumber>> {
    loop {
        if *stop_receiver.borrow() {
            return Ok(None);
        }

        let mut storage = pool.access_storage().await;
        let sealed_l1_batch_number = storage.blocks_dal().get_earliest_l1_batch_number().await?;
        drop(storage);

        if let Some(number) = sealed_l1_batch_number {
            return Ok(Some(number));
        }
        tracing::debug!("No L1 batches are present in DB; trying again in {poll_interval:?}");

        // We don't check the result: if a stop signal is received, we'll return at the start
        // of the next iteration.
        tokio::time::timeout(poll_interval, stop_receiver.changed())
            .await
            .ok();
    }
}
