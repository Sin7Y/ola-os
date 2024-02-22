-- Add down migration script here
DROP TABLE IF EXISTS prover_jobs_fri;
DROP INDEX IF EXISTS prover_jobs_fri_composite_index_1;
DROP INDEX IF EXISTS idx_prover_jobs_fri_queued_order;
DROP INDEX IF EXISTS idx_prover_jobs_fri_queued_order2;
DROP INDEX IF EXISTS idx_witness_inputs_fri_status_processing_attempts;
DROP INDEX IF EXISTS prover_jobs_fri_status_processing_started_at_idx_2;
DROP INDEX IF EXISTS idx_prover_jobs_fri_status;
DROP INDEX IF EXISTS idx_prover_jobs_fri_circuit_id_agg_batch_num;