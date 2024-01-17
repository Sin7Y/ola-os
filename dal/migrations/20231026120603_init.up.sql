CREATE TABLE storage
(
    hashed_key BYTEA PRIMARY KEY,
    value   BYTEA NOT NULL,
    tx_hash BYTEA NOT NULL,
    address BYTEA NOT NULL,
    key     BYTEA NOT NULL,

    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
)
    WITH (fillfactor = 50);

CREATE TABLE l1_batches
(
    number BIGSERIAL PRIMARY KEY,
    timestamp BIGINT NOT NULL,
    is_finished BOOL NOT NULL,
    l1_tx_count INT NOT NULL,
    l2_tx_count INT NOT NULL,

    hash BYTEA,
    parent_hash BYTEA,
    commitment BYTEA,
    compressed_write_logs BYTEA,
    compressed_contracts BYTEA,
    
    merkle_root_hash BYTEA,
    initial_bootloader_heap_content JSONB NOT NULL,
    used_contract_hashes JSONB NOT NULL,
    compressed_initial_writes BYTEA,
    compressed_repeated_writes BYTEA,
    rollup_last_leaf_index BIGINT,
    bootloader_code_hash BYTEA,
    default_aa_code_hash BYTEA,

    aux_data_hash BYTEA,
    pass_through_data_hash BYTEA,
    meta_parameters_hash BYTEA,

    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);

CREATE INDEX blocks_hash ON l1_batches USING hash (hash);

CREATE TABLE miniblocks (
    number BIGSERIAL PRIMARY KEY,
    l1_batch_number BIGINT,
    timestamp BIGINT NOT NULL,
    hash BYTEA NOT NULL,

    l1_tx_count INT NOT NULL,
    l2_tx_count INT NOT NULL,

    bootloader_code_hash BYTEA,
    default_aa_code_hash BYTEA,

    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
CREATE INDEX miniblocks_l1_batch_number_idx ON miniblocks (l1_batch_number);
CREATE INDEX miniblocks_hash ON miniblocks USING hash (hash);
CREATE INDEX ix_miniblocks_t1 ON miniblocks USING btree (number) INCLUDE (l1_batch_number, "timestamp");

CREATE TABLE transactions
(
    hash BYTEA PRIMARY KEY,
    is_priority BOOLEAN NOT NULL,
    initiator_address BYTEA NOT NULL,
    nonce BIGINT,
    signature BYTEA,
    input BYTEA,
    data JSONB NOT NULL,
    received_at TIMESTAMP NOT NULL,
    priority_op_id BIGINT,

    l1_batch_number BIGINT REFERENCES l1_batches (number) ON DELETE SET NULL,
    l1_block_number INT,
    miniblock_number BIGINT,
    index_in_block INT,
    error VARCHAR,

    tx_format INTEGER,
    execution_info JSONB NOT NULL DEFAULT '{}',
    contract_address BYTEA,
    in_mempool BOOLEAN NOT NULL default false,
    l1_batch_tx_index INT,
    upgrade_id INT,

    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);

ALTER TABLE transactions ADD CONSTRAINT transactions_miniblock_number_fkey
    FOREIGN KEY (miniblock_number) REFERENCES miniblocks (number);

CREATE INDEX transactions_received_at_idx ON transactions(received_at);
CREATE UNIQUE INDEX transactions_initiator_address_nonce ON transactions (initiator_address, nonce);
CREATE INDEX transactions_priority_op_id_idx ON transactions (priority_op_id) WHERE priority_op_id IS NOT NULL;
CREATE INDEX transactions_contract_address_idx ON transactions (contract_address);
CREATE INDEX transactions_in_mempool_idx ON transactions (in_mempool) WHERE in_mempool = TRUE;
CREATE INDEX transactions_l1_batch_number_idx ON transactions (l1_batch_number);
CREATE INDEX transactions_miniblock_number_tx_index_idx ON transactions (miniblock_number, index_in_block);
CREATE INDEX pending_l1_batch_txs ON transactions USING btree (miniblock_number, index_in_block) WHERE ((miniblock_number IS NOT NULL) AND (l1_batch_number IS NULL));

CREATE TABLE protocol_versions (
    id INT PRIMARY KEY,
    timestamp BIGINT NOT NULL,
    bootloader_code_hash BYTEA NOT NULL,
    default_account_code_hash BYTEA NOT NULL,
    upgrade_tx_hash BYTEA REFERENCES transactions (hash),
    created_at TIMESTAMP NOT NULL
);

ALTER TABLE l1_batches ADD COLUMN IF NOT EXISTS protocol_version INT REFERENCES protocol_versions (id);
ALTER TABLE miniblocks ADD COLUMN IF NOT EXISTS protocol_version INT REFERENCES protocol_versions (id);

CREATE TABLE storage_logs
(
    hashed_key BYTEA NOT NULL,
    address BYTEA NOT NULL,
    key BYTEA NOT NULL,
    value BYTEA NOT NULL,
    operation_number INT NOT NULL,
    tx_hash BYTEA NOT NULL,
    miniblock_number BIGINT NOT NULL REFERENCES miniblocks (number) ON DELETE CASCADE,

    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);

ALTER TABLE storage_logs ADD PRIMARY KEY (hashed_key, miniblock_number, operation_number);

CREATE INDEX storage_logs_miniblock_number_idx ON storage_logs (miniblock_number);
-- This is the ACCOUNT_CODE_STORAGE address.
-- TODO: replace address
CREATE INDEX storage_logs_contract_address_tx_hash_idx_upd ON storage_logs (tx_hash) WHERE (address = '\x0000000000000000000000000000000000000000000000000000000000008002'::bytea);


CREATE TABLE transaction_traces
(
    tx_hash BYTEA PRIMARY KEY,
    trace JSONB NOT NULL,

    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);

CREATE TABLE factory_deps
(
    bytecode_hash BYTEA PRIMARY KEY,
    bytecode BYTEA NOT NULL,
    miniblock_number BIGINT NOT NULL REFERENCES miniblocks (number) ON DELETE CASCADE,

    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);


CREATE TABLE protective_reads (
    l1_batch_number BIGINT REFERENCES l1_batches (number) ON DELETE CASCADE,
    address BYTEA NOT NULL,
    key BYTEA NOT NULL,

    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    PRIMARY KEY (address, key, l1_batch_number)
);

CREATE INDEX protective_reads_l1_batch_number_index ON protective_reads (l1_batch_number);

CREATE TABLE initial_writes (
    hashed_key BYTEA NOT NULL PRIMARY KEY,
    l1_batch_number BIGINT NOT NULL REFERENCES l1_batches (number) ON DELETE CASCADE,
    index BIGINT,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);

CREATE INDEX initial_writes_l1_batch_number_index ON initial_writes (l1_batch_number);
CREATE UNIQUE INDEX initial_writes_index_index ON initial_writes (index);
CREATE INDEX ix_initial_writes_t1 ON initial_writes USING btree (hashed_key) INCLUDE (l1_batch_number);

CREATE TABLE call_traces (
    tx_hash   BYTEA PRIMARY KEY,
    call_trace BYTEA NOT NULL,
    FOREIGN KEY (tx_hash) REFERENCES transactions (hash) ON DELETE CASCADE
);

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
