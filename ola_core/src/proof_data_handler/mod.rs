use std::{net::SocketAddr, sync::Arc};

use anyhow::{Context, Ok};
use axum::{extract::Path, routing::post, Json, Router};
use ola_config::proof_data_handler::ProofDataHandlerConfig;
use ola_dal::connection::ConnectionPool;
use ola_types::prover_server_api::{ProofGenerationDataRequest, SubmitProofRequest};
use olaos_object_store::ObjectStore;
use tokio::sync::watch;

use crate::proof_data_handler::request_processor::RequestProcessor;

mod request_processor;

pub(crate) async fn run_server(
    config: ProofDataHandlerConfig,
    blob_store: Arc<dyn ObjectStore>,
    pool: ConnectionPool,
    mut stop_receiver: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let bind_address = SocketAddr::from(([0, 0, 0, 0], config.http_port));
    olaos_logs::debug!("Starting proof data handler server on {bind_address}");
    let get_proof_gen_processor = RequestProcessor::new(blob_store, pool, config);
    let submit_proof_processor = get_proof_gen_processor.clone();
    let app = Router::new()
        .route(
            "/proof_generation_data",
            post(
                // we use post method because the returned data is not idempotent,
                // i.e we return different result on each call.
                |payload: Json<ProofGenerationDataRequest>| async move {
                    get_proof_gen_processor
                        .get_proof_generation_data(payload)
                        .await
                },
            ),
        )
        .route(
            "/submit_proof/:l1_batch_number",
            post(
                move |l1_batch_number: Path<u32>, payload: Json<SubmitProofRequest>| async move {
                    submit_proof_processor
                        .submit_proof(l1_batch_number, payload)
                        .await
                },
            ),
        );

    axum::Server::bind(&bind_address)
        .serve(app.into_make_service())
        .with_graceful_shutdown(async move {
            if stop_receiver.changed().await.is_err() {
                olaos_logs::warn!("Stop signal sender for proof data handler server was dropped without sending a signal");
            }
            olaos_logs::info!("Stop signal received, proof data handler server is shutting down");
        })
        .await
        .context("Proof data handler server failed")?;
    olaos_logs::info!("Proof data handler server shut down");
    Ok(())
}
