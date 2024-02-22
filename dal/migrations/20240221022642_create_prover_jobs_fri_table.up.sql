-- Add up migration script here
CREATE TABLE IF NOT EXISTS prover_jobs_fri
(
    id BIGSERIAL PRIMARY KEY,
    l1_batch_number BIGINT NOT NULL,
    circuit_id SMALLINT NOT NULL,
    circuit_blob_url TEXT NOT NULL,
    aggregation_round SMALLINT NOT NULL,
    sequence_number INT NOT NULL,
    proof_blob_url TEXT,
    status TEXT NOT NULL,
    depth INT NOT NULL DEFAULT 0,
    error TEXT,
    attempts SMALLINT NOT NULL DEFAULT 0,
    processing_started_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    time_taken TIME,
    is_blob_cleaned BOOLEAN,
    is_node_final_proof BOOLEAN NOT NULL DEFAULT FALSE,
    protocol_version INT REFERENCES prover_fri_protocol_versions (id),
    picked_by TEXT
    );

CREATE UNIQUE INDEX IF NOT EXISTS prover_jobs_fri_composite_index_1 ON prover_jobs_fri(l1_batch_number, aggregation_round, circuit_id, depth, sequence_number) INCLUDE(protocol_version);

CREATE INDEX IF NOT EXISTS idx_prover_jobs_fri_queued_order
    ON prover_jobs_fri (aggregation_round DESC, l1_batch_number ASC, id ASC)
    WHERE status = 'queued';

CREATE INDEX IF NOT EXISTS idx_prover_jobs_fri_queued_order2 ON prover_jobs_fri USING btree (l1_batch_number, aggregation_round DESC, id)
    WHERE status = 'queued';

CREATE INDEX IF NOT EXISTS idx_witness_inputs_fri_status_processing_attempts
    ON witness_inputs_fri (processing_started_at, attempts)
    WHERE status IN ('in_progress', 'failed');

CREATE INDEX IF NOT EXISTS prover_jobs_fri_status_processing_started_at_idx_2 ON prover_jobs_fri (status, processing_started_at)
    WHERE (attempts < 20);

CREATE INDEX IF NOT EXISTS idx_prover_jobs_fri_status ON prover_jobs_fri (circuit_id, aggregation_round)
    WHERE (status != 'successful' and status != 'skipped');

CREATE INDEX IF NOT EXISTS idx_prover_jobs_fri_circuit_id_agg_batch_num
    ON prover_jobs_fri (circuit_id, aggregation_round, l1_batch_number)
    WHERE status IN ('queued', 'in_progress', 'in_gpu_proof', 'failed');