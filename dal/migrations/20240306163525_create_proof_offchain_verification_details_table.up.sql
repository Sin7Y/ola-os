CREATE TABLE IF NOT EXISTS proof_offchain_verification_details
(
    l1_batch_number         BIGINT PRIMARY KEY REFERENCES l1_batches (number) ON DELETE CASCADE,
    status                  TEXT      NOT NULL,
    verifier_picked_at      TIMESTAMP,
    verifier_submit_at      TIMESTAMP,
    created_at              TIMESTAMP NOT NULL,
    updated_at              TIMESTAMP NOT NULL
);


CREATE INDEX IF NOT EXISTS idx_proof_offchain_verification_details_status_verifier_picked_at
    ON proof_offchain_verification_details (verifier_picked_at)
    WHERE status = 'picked_by_offchain_verifier';