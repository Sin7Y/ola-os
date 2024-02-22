use std::sync::Arc;

use axum::{
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use ola_config::proof_data_handler::{ProofDataHandlerConfig, ProtocolVersionLoadingMode};
use ola_dal::{connection::ConnectionPool, SqlxError};
use ola_types::{
    proofs::PrepareBasicCircuitsJob,
    protocol_version::{FriProtocolVersionId, L1VerifierConfig},
    prover_server_api::{
        ProofGenerationData, ProofGenerationDataRequest, ProofGenerationDataResponse,
        SubmitProofRequest, SubmitProofResponse,
    },
    L1BatchNumber,
};
use olaos_object_store::{ObjectStore, ObjectStoreError};

pub(crate) enum RequestProcessorError {
    ObjectStore(ObjectStoreError),
    Sqlx(SqlxError),
}

impl IntoResponse for RequestProcessorError {
    fn into_response(self) -> Response {
        let (status_code, message) = match self {
            RequestProcessorError::ObjectStore(err) => {
                olaos_logs::error!("GCS error: {:?}", err);
                (
                    StatusCode::BAD_GATEWAY,
                    "Failed fetching/saving from GCS".to_owned(),
                )
            }
            RequestProcessorError::Sqlx(err) => {
                olaos_logs::error!("Sqlx error: {:?}", err);
                match err {
                    SqlxError::RowNotFound => {
                        (StatusCode::NOT_FOUND, "Non existing L1 batch".to_owned())
                    }
                    _ => (
                        StatusCode::BAD_GATEWAY,
                        "Failed fetching/saving from db".to_owned(),
                    ),
                }
            }
        };
        (status_code, message).into_response()
    }
}

#[derive(Clone)]
pub(crate) struct RequestProcessor {
    blob_store: Arc<dyn ObjectStore>,
    pool: ConnectionPool,
    config: ProofDataHandlerConfig,
    l1_verifier_config: Option<L1VerifierConfig>,
}

impl RequestProcessor {
    pub(crate) fn new(
        blob_store: Arc<dyn ObjectStore>,
        pool: ConnectionPool,
        config: ProofDataHandlerConfig,
        l1_verifier_config: Option<L1VerifierConfig>,
    ) -> Self {
        Self {
            blob_store,
            pool,
            config,
            l1_verifier_config,
        }
    }

    pub(crate) async fn get_proof_generation_data(
        &self,
        request: Json<ProofGenerationDataRequest>,
    ) -> Result<Json<ProofGenerationDataResponse>, RequestProcessorError> {
        olaos_logs::info!("Received request for proof generation data: {:?}", request);

        let l1_batch_number_result = self
            .pool
            .access_storage()
            .await
            .proof_generation_dal()
            .get_next_block_to_be_proven(self.config.proof_generation_timeout())
            .await;

        let l1_batch_number = match l1_batch_number_result {
            Some(number) => number,
            None => return Ok(Json(ProofGenerationDataResponse::Success(None))), // no batches pending to be proven
        };

        let blob = self
            .blob_store
            .get(l1_batch_number)
            .await
            .map_err(RequestProcessorError::ObjectStore)?;

        let fri_protocol_version_id =
            FriProtocolVersionId::try_from(self.config.fri_protocol_version_id)
                .expect("Invalid FRI protocol version id");

        let l1_verifier_config= match self.config.protocol_version_loading_mode {
            ProtocolVersionLoadingMode::FromDb => {
                panic!("Loading protocol version from db is not implemented yet")
            }
            ProtocolVersionLoadingMode::FromEnvVar => {
                self.l1_verifier_config
                    .expect("l1_verifier_config must be set while running ProtocolVersionLoadingMode::FromEnvVar mode")
            }
        };

        let proof_gen_data = ProofGenerationData {
            l1_batch_number,
            data: blob,
            fri_protocol_version_id,
            l1_verifier_config,
        };

        Ok(Json(ProofGenerationDataResponse::Success(Some(
            proof_gen_data,
        ))))
    }

    pub(crate) async fn submit_proof(
        &self,
        Path(l1_batch_number): Path<u32>,
        Json(_payload): Json<SubmitProofRequest>,
    ) -> Result<Json<SubmitProofResponse>, RequestProcessorError> {
        let _l1_batch_number = L1BatchNumber(l1_batch_number);
        Ok(Json(SubmitProofResponse::Success))
    }
}
