-- Add down migration script here
DROP TABLE prover_fri_protocol_versions;

DROP TABLE IF EXISTS witness_inputs_fri;
DROP INDEX IF EXISTS idx_witness_inputs_fri_queued_order;
DROP INDEX IF EXISTS idx_witness_inputs_fri_status_processing_attempts;