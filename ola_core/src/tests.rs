#[cfg(test)]
mod tests {
    use std::{collections::HashMap, fs::File, io::Read};

    use ola_config::constants::contracts::*;
    use ola_dal::StorageProcessor;
    use ola_state::rocksdb::RocksdbStorage;
    use ola_types::{
        get_full_code_key,
        log::{LogQuery, StorageLog, StorageLogKind, Timestamp},
        AccountTreeId, Address, L1BatchNumber, MiniblockNumber, StorageKey, H256, U256,
    };
    use ola_utils::{h256_to_u256, hash::PoseidonBytes, u256_to_h256};
    use olavm_core::{
        crypto::poseidon_trace::calculate_arbitrary_poseidon,
        program::binary_program::BinaryProgram, types::GoldilocksField,
    };

    use crate::sequencer::io::sort_storage_access::sort_storage_access_queries;

    #[ignore]
    #[tokio::test]
    async fn manually_depoly_contract() {
        let address = u256_to_h256(U256([100, 100, 100, 100]));
        let path = "example/vote_simple_bin.json".to_string();
        let mut program_file = File::open(path.clone()).unwrap();
        let program: BinaryProgram = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        let mut program_bytes = Vec::new();
        let _ = program_file.read_to_end(&mut program_bytes).unwrap();
        let program_hash = H256(program_bytes.hash_bytes());
        let instructions_u64 = program.bytecode_u64_array().unwrap();
        let instructions: Vec<GoldilocksField> = instructions_u64
            .iter()
            .map(|n| GoldilocksField(*n))
            .collect();
        let bytecode_hash_u256 = calculate_arbitrary_poseidon(&instructions).map(|fe| fe.0);
        let bytecode_hash = u256_to_h256(U256(bytecode_hash_u256));
        println!(
            "prog_hash: {:?}, bytecode_hash: {:?}",
            program_hash.0, bytecode_hash.0
        );
        let prog_hash_key = get_full_code_key(&address);
        let bytecode_key = prog_hash_key.add(1);
        let storage_logs = vec![(
            H256::default(),
            vec![
                StorageLog::new_write_log(prog_hash_key, program_hash),
                StorageLog::new_write_log(bytecode_key, bytecode_hash),
            ],
        )];

        let mut storage: StorageProcessor<'_> = StorageProcessor::establish_connection(true).await;
        let mut transaction = storage.start_transaction().await;
        transaction
            .storage_logs_dal()
            .insert_storage_logs(MiniblockNumber(0), &storage_logs)
            .await;

        let log_queries: Vec<LogQuery> = storage_logs
            .iter()
            .enumerate()
            .flat_map(|(tx_index, (_, storage_logs))| {
                storage_logs
                    .iter()
                    .enumerate()
                    .map(move |(log_index, storage_log)| {
                        LogQuery {
                            // Monotonically increasing Timestamp. Normally it's generated by the VM, but we don't have a VM in the genesis block.
                            timestamp: Timestamp(((tx_index << 16) + log_index) as u32),
                            tx_number_in_block: tx_index as u16,
                            aux_byte: 0,
                            shard_id: 0,
                            address: *storage_log.key.address(),
                            key: h256_to_u256(*storage_log.key.key()),
                            read_value: h256_to_u256(H256::zero()),
                            written_value: h256_to_u256(storage_log.value),
                            rw_flag: storage_log.kind == StorageLogKind::Write,
                            rollback: false,
                            is_service: false,
                        }
                    })
                    .collect::<Vec<LogQuery>>()
            })
            .collect();

        let (_, deduped_log_queries) = sort_storage_access_queries(&log_queries);

        let (deduplicated_writes, protective_reads): (Vec<_>, Vec<_>) = deduped_log_queries
            .into_iter()
            .partition(|log_query| log_query.rw_flag);
        transaction
            .storage_logs_dedup_dal()
            .insert_protective_reads(L1BatchNumber(0), &protective_reads)
            .await;

        let written_storage_keys: Vec<_> = deduplicated_writes
            .iter()
            .map(|log| StorageKey::new(AccountTreeId::new(log.address), u256_to_h256(log.key)))
            .collect();
        transaction
            .storage_logs_dedup_dal()
            .insert_initial_writes(L1BatchNumber(0), &written_storage_keys)
            .await;

        transaction
            .storage_dal()
            .apply_storage_logs(&storage_logs)
            .await;

        let mut factory_deps = HashMap::new();
        factory_deps.insert(program_hash, program_bytes);
        transaction
            .storage_dal()
            .insert_factory_deps(MiniblockNumber(0), &factory_deps)
            .await;

        transaction.commit().await;
    }

    #[tokio::test]
    async fn rocks_read() {
        let mut storage: StorageProcessor<'_> = StorageProcessor::establish_connection(true).await;

        let path = "../db/main/sequencer".to_string();
        let mut db = RocksdbStorage::new(path.as_ref());

        db.update_from_postgres(&mut storage).await;

        let l1_number = db.l1_batch_number();
        println!("l1_number: {:?}", l1_number);

        // ("", "AccountCodeStorage", ACCOUNT_CODE_STORAGE_ADDRESS),
        // ("", "NonceHolder", NONCE_HOLDER_ADDRESS),
        // ("", "KnownCodesStorage", KNOWN_CODES_STORAGE_ADDRESS),
        // ("", "ContractDeployer", CONTRACT_DEPLOYER_ADDRESS),
        // ("", "VoteSimple", SIMPLE_VOTE_ADDRESS),

        let storage_key = get_full_code_key(&ENTRYPOINT_ADDRESS);
        let v = db.read_value_inner(&storage_key);
        match v {
            Some(v) => {
                println!("v: {:?}", v);
            }
            None => {
                println!("value not found");
            }
        }
    }
}
