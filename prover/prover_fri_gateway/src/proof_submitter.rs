use async_trait::async_trait;
use ola_dal::fri_prover_dal::FriProofJobStatus;
use ola_types::{
    proofs::L1BatchProofForL1,
    prover_server_api::{SubmitProofRequest, SubmitProofResponse},
    L1BatchNumber,
};
use olaos_prover_fri_types::FriProofWrapper;

use crate::api_data_fetcher::{PeriodicApi, PeriodicApiStruct};

impl PeriodicApiStruct {
    async fn next_submit_proof_request(&self) -> Option<(L1BatchNumber, SubmitProofRequest)> {
        let (l1_batch_number, status) = self
            .pool
            .access_storage()
            .await
            .fri_prover_jobs_dal()
            .get_least_proven_block_number_not_sent_to_server()
            .await?;

        let request = match status {
            FriProofJobStatus::Successful => {
                let proof: FriProofWrapper = self
                    .blob_store
                    .get(l1_batch_number)
                    .await
                    .expect("Failed to get compressed snark proof from blob store");
                let data = bincode::serialize(&proof).unwrap();
                let l1_batch_proof = L1BatchProofForL1 { proof: data };
                SubmitProofRequest::Proof(Box::new(l1_batch_proof))
            }
            FriProofJobStatus::Skipped => SubmitProofRequest::SkippedProofGeneration,
            _ => panic!(
                "Trying to send proof that are not successful status: {:?}",
                status
            ),
        };

        Some((l1_batch_number, request))
    }

    async fn save_successful_sent_proof(&self, l1_batch_number: L1BatchNumber) {
        self.pool
            .access_storage()
            .await
            .fri_prover_jobs_dal()
            .mark_proof_sent_to_server(l1_batch_number)
            .await;
    }
}

#[async_trait]
impl PeriodicApi<SubmitProofRequest> for PeriodicApiStruct {
    type JobId = L1BatchNumber;
    type Response = SubmitProofResponse;
    const SERVICE_NAME: &'static str = "ProofSubmitter";

    async fn get_next_request(&self) -> Option<(Self::JobId, SubmitProofRequest)> {
        let (l1_batch_number, request) = self.next_submit_proof_request().await?;
        Some((l1_batch_number, request))
    }

    async fn send_request(
        &self,
        job_id: Self::JobId,
        request: SubmitProofRequest,
    ) -> reqwest::Result<Self::Response> {
        let endpoint = format!("{}/{job_id}", self.api_url);
        self.send_http_request(request, &endpoint).await
    }

    async fn handle_response(&self, job_id: L1BatchNumber, response: Self::Response) {
        olaos_logs::info!("Received response: {:?}", response);
        self.save_successful_sent_proof(job_id).await;
    }
}
