DROP TABLE IF EXISTS call_traces;

DROP INDEX IF EXISTS ix_initial_writes_t1;
DROP INDEX IF EXISTS initial_writes_index_index;
DROP INDEX IF EXISTS initial_writes_l1_batch_number_index;
DROP TABLE IF EXISTS initial_writes;

DROP INDEX IF EXISTS protective_reads_l1_batch_number_index;
DROP TABLE protective_reads;

DROP TABLE factory_deps;

DROP TABLE transaction_traces;

DROP INDEX storage_logs_contract_address_tx_hash_idx_upd;
DROP INDEX storage_logs_miniblock_number_idx;
DROP TABLE storage_logs;

DROP TABLE protocol_versions;

DROP INDEX pending_l1_batch_txs;
DROP INDEX transactions_miniblock_number_tx_index_idx;
DROP INDEX transactions_l1_batch_number_idx;
DROP INDEX transactions_in_mempool_idx;
DROP INDEX transactions_contract_address_idx;
DROP INDEX transactions_priority_op_id_idx;
DROP INDEX transactions_initiator_address_nonce;
DROP INDEX transactions_received_at_idx;
DROP TABLE transactions;


DROP INDEX blocks_hash;
DROP TABLE l1_batches;

DROP INDEX IF EXISTS ix_miniblocks_t1;
DROP INDEX miniblocks_hash;
DROP INDEX miniblocks_l1_batch_number_idx;
DROP TABLE miniblocks;

DROP TABLE storage;