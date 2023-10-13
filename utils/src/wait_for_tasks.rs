use futures::{future, Future};
use tokio::task::JoinHandle;

use crate::panic_extractor::try_extract_panic_message;

pub async fn wait_for_tasks<Fut>(
    task_futures: Vec<JoinHandle<()>>,
    graceful_shutdown: Option<Fut>,
    tasks_allowed_to_finish: bool,
) where
    Fut: Future<Output = ()>,
{
    match future::select_all(task_futures).await.0 {
        Ok(_) => {
            if tasks_allowed_to_finish {
                olaos_logs::error!("One of the actors finished its run. Finishing execution.");
            } else {
                olaos_logs::info!(
                    "One of the actors finished its run, while it wasn't expected to do it"
                );
                if let Some(graceful_shutdown) = graceful_shutdown {
                    graceful_shutdown.await;
                }
            }
        }
        Err(error) => {
            let panic_message = try_extract_panic_message(error);

            olaos_logs::error!(
                "One of the tokio actors unexpectedly finished with error: {panic_message}"
            );

            if let Some(graceful_shutdown) = graceful_shutdown {
                graceful_shutdown.await;
            }
        }
    }
}
