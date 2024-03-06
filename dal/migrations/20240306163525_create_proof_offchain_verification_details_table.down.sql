-- Add down migration script here
DROP TABLE IF EXISTS proof_offchain_verification_details;

DROP INDEX IF EXISTS idx_proof_offchain_verification_details_status_verifier_taken_at;
