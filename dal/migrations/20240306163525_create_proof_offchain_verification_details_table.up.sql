-- Add up migration script here
CREATE TABLE IF NOT EXISTS proof_offchain_verification_details
(
    l1_batch_number         BIGINT PRIMARY KEY REFERENCES l1_batches (number) ON DELETE CASCADE,
    status                  TEXT      NOT NULL,
    created_at              TIMESTAMP NOT NULL,
    updated_at              TIMESTAMP NOT NULL,
    verifier_taken_at       TIMESTAMP
);


CREATE INDEX IF NOT EXISTS idx_proof_offchain_verification_details_status_verifier_taken_at
    ON proof_offchain_verification_details (verifier_taken_at)
    WHERE status = 'picked_by_verifier';

