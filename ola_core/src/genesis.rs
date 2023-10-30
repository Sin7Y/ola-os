use ola_contracts::BaseSystemContracts;
use ola_dal::StorageProcessor;
use ola_types::{
    block::{DeployedContract, L1BatchHeader, MiniblockHeader},
    // commitment::L1BatchCommitment,
    get_code_key,
    get_system_context_init_logs,
    log::{LogQuery, StorageLog, StorageLogKind, Timestamp},
    protocol_version::{ProtocolVersion, ProtocolVersionId},
    AccountTreeId,
    L1BatchNumber,
    L2ChainId,
    MiniblockNumber,
    StorageKey,
    H256,
};
use ola_utils::{
    be_words_to_bytes, bytecode::hash_bytecode, h256_to_u256, misc::miniblock_hash, u256_to_h256,
};

use crate::sequencer::io::sort_storage_access::sort_storage_access_queries;

// use crate::metadata_calculator::helpers::L1BatchWithLogs;

#[derive(Debug, Clone)]
pub struct GenesisParams {
    pub base_system_contracts: BaseSystemContracts,
    pub system_contracts: Vec<DeployedContract>,
}

pub async fn ensure_genesis_state(
    storage: &mut StorageProcessor<'_>,
    ola_chain_id: L2ChainId,
    genesis_params: &GenesisParams,
) -> H256 {
    let mut transaction = storage.start_transaction().await;

    // return if genesis block was already processed
    if !transaction.blocks_dal().is_genesis_needed().await {
        olaos_logs::debug!("genesis is not needed!");
        return transaction
            .blocks_dal()
            .get_l1_batch_state_root(L1BatchNumber(0))
            .await
            .expect("genesis block hash is empty");
    }

    olaos_logs::info!("running regenesis");
    let GenesisParams {
        base_system_contracts,
        system_contracts,
    } = genesis_params;

    let base_system_contracts_hashes = base_system_contracts.hashes();

    create_genesis_l1_batch(
        &mut transaction,
        ola_chain_id,
        base_system_contracts,
        system_contracts,
    )
    .await;
    olaos_logs::info!("chain_schema_genesis is complete");

    // TODO:
    // let storage_logs = L1BatchWithLogs::new(&mut transaction, L1BatchNumber(0)).await;
    // let storage_logs = storage_logs.unwrap().storage_logs;
    // let metadata = ZkSyncTree::process_genesis_batch(&storage_logs);
    // let genesis_root_hash = metadata.root_hash;
    // let rollup_last_leaf_index = metadata.leaf_count + 1;

    // let block_commitment = L1BatchCommitment::new(
    //     vec![],
    //     rollup_last_leaf_index,
    //     genesis_root_hash,
    //     vec![],
    //     vec![],
    //     base_system_contracts_hashes.bootloader,
    //     base_system_contracts_hashes.default_aa,
    // );

    // save_genesis_l1_batch_metadata(
    //     &mut transaction,
    //     &block_commitment,
    //     genesis_root_hash,
    //     rollup_last_leaf_index,
    // )
    // .await;
    olaos_logs::info!("operations_schema_genesis is complete");

    transaction.commit().await;

    // We need to `println` this value because it will be used to initialize the smart contract.
    // TODO:
    // println!("CONTRACTS_GENESIS_ROOT={:?}", genesis_root_hash);
    // println!(
    //     "CONTRACTS_GENESIS_BLOCK_COMMITMENT={:?}",
    //     block_commitment.hash().commitment
    // );
    // println!(
    //     "CONTRACTS_GENESIS_ROLLUP_LEAF_INDEX={}",
    //     rollup_last_leaf_index
    // );
    println!(
        "CHAIN_SEQUENCER_BOOTLOADER_HASH={:?}",
        base_system_contracts_hashes.bootloader
    );
    println!(
        "CHAIN_SEQUENCER_DEFAULT_AA_HASH={:?}",
        base_system_contracts_hashes.default_aa
    );

    H256::default()
    // genesis_root_hash
}

pub(crate) async fn create_genesis_l1_batch(
    storage: &mut StorageProcessor<'_>,
    chain_id: L2ChainId,
    base_system_contracts: &BaseSystemContracts,
    system_contracts: &[DeployedContract],
) {
    let version = ProtocolVersion {
        id: ProtocolVersionId::latest(),
        timestamp: 0,
        base_system_contracts_hashes: base_system_contracts.hashes(),
        tx: None,
    };

    let mut genesis_l1_batch_header = L1BatchHeader::new(
        L1BatchNumber(0),
        0,
        base_system_contracts.hashes(),
        ProtocolVersionId::latest(),
    );
    genesis_l1_batch_header.is_finished = true;

    let genesis_miniblock_header = MiniblockHeader {
        number: MiniblockNumber(0),
        timestamp: 0,
        hash: miniblock_hash(MiniblockNumber(0)),
        l1_tx_count: 0,
        l2_tx_count: 0,
        base_system_contracts_hashes: base_system_contracts.hashes(),
        protocol_version: Some(ProtocolVersionId::latest()),
    };

    let mut transaction = storage.start_transaction().await;

    transaction
        .protocol_versions_dal()
        .save_protocol_version(version)
        .await;
    transaction
        .blocks_dal()
        .insert_l1_batch(&genesis_l1_batch_header, &[])
        .await;
    transaction
        .blocks_dal()
        .insert_miniblock(&genesis_miniblock_header)
        .await;
    transaction
        .blocks_dal()
        .mark_miniblocks_as_executed_in_l1_batch(L1BatchNumber(0))
        .await;

    insert_base_system_contracts_to_factory_deps(&mut transaction, base_system_contracts).await;
    insert_system_contracts(&mut transaction, system_contracts, chain_id).await;

    transaction.commit().await;
}

// pub(crate) async fn save_genesis_l1_batch_metadata(
//     storage: &mut StorageProcessor<'_>,
//     commitment: &L1BatchCommitment,
//     genesis_root_hash: H256,
//     rollup_last_leaf_index: u64,
// ) {
//     let commitment_hash = commitment.hash();

//     let metadata = L1BatchMetadata {
//         root_hash: genesis_root_hash,
//         rollup_last_leaf_index,
//         merkle_root_hash: genesis_root_hash,
//         initial_writes_compressed: vec![],
//         repeated_writes_compressed: vec![],
//         commitment: commitment_hash.commitment,
//         block_meta_params: commitment.meta_parameters(),
//         aux_data_hash: commitment_hash.aux_output,
//         meta_parameters_hash: commitment_hash.meta_parameters,
//         pass_through_data_hash: commitment_hash.pass_through_data,
//     };
//     storage
//         .blocks_dal()
//         .save_genesis_l1_batch_metadata(&metadata)
//         .await;
// }

async fn insert_base_system_contracts_to_factory_deps(
    storage: &mut StorageProcessor<'_>,
    contracts: &BaseSystemContracts,
) {
    let factory_deps = [&contracts.entrypoint, &contracts.default_aa]
        .iter()
        .map(|c| (c.hash, be_words_to_bytes(&c.code)))
        .collect();

    storage
        .storage_dal()
        .insert_factory_deps(MiniblockNumber(0), &factory_deps)
        .await;
}

async fn insert_system_contracts(
    storage: &mut StorageProcessor<'_>,
    contracts: &[DeployedContract],
    chain_id: L2ChainId,
) {
    let system_context_init_logs = (H256::default(), get_system_context_init_logs(chain_id));

    let storage_logs: Vec<(H256, Vec<StorageLog>)> = contracts
        .iter()
        .map(|contract| {
            let hash = hash_bytecode(&contract.bytecode);
            let code_key = get_code_key(contract.account_id.address());

            (
                Default::default(),
                vec![StorageLog::new_write_log(code_key, hash)],
            )
        })
        .chain(Some(system_context_init_logs))
        .collect();

    let mut transaction = storage.start_transaction().await;

    transaction
        .storage_logs_dal()
        .insert_storage_logs(MiniblockNumber(0), &storage_logs)
        .await;

    // we don't produce proof for the genesis block,
    // but we still need to populate the table
    // to have the correct initial state of the merkle tree
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

    let factory_deps = contracts
        .iter()
        .map(|c| (hash_bytecode(&c.bytecode), c.bytecode.clone()))
        .collect();
    transaction
        .storage_dal()
        .insert_factory_deps(MiniblockNumber(0), &factory_deps)
        .await;

    transaction.commit().await;
}
