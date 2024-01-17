-- Add up migration script here
CREATE TABLE IF NOT EXISTS prover_fri_protocol_versions (
    id INT PRIMARY KEY,
    created_at TIMESTAMP NOT NULL
);

CREATE TABLE IF NOT EXISTS witness_inputs_fri
(
    l1_batch_number BIGINT NOT NULL PRIMARY KEY,
    merkle_tree_paths_blob_url TEXT,
    attempts SMALLINT NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    error TEXT,
    picked_by TEXT,
    protocol_version INT REFERENCES prover_fri_protocol_versions (id),
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    processing_started_at TIMESTAMP,
    time_taken TIME,
    is_blob_cleaned BOOLEAN
);
CREATE INDEX IF NOT EXISTS idx_witness_inputs_fri_status_processing_attempts
    ON witness_inputs_fri (processing_started_at, attempts)
    WHERE status IN ('in_progress', 'failed');
CREATE INDEX IF NOT EXISTS idx_witness_inputs_fri_queued_order
    ON witness_inputs_fri (l1_batch_number ASC)
    WHERE status = 'queued';