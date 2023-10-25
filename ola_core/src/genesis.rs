use ola_contracts::BaseSystemContracts;
use ola_dal::StorageProcessor;
use ola_types::{
    block::DeployedContract,
    commitment::L1BatchCommitment,
    protocol_version::{ProtocolVersion, ProtocolVersionId},
    H256,
};

use crate::metadata_calculator::helpers::L1BatchWithLogs;

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

    let storage_logs = L1BatchWithLogs::new(&mut transaction, L1BatchNumber(0)).await;
    let storage_logs = storage_logs.unwrap().storage_logs;
    let metadata = ZkSyncTree::process_genesis_batch(&storage_logs);
    let genesis_root_hash = metadata.root_hash;
    let rollup_last_leaf_index = metadata.leaf_count + 1;

    // let block_commitment = L1BatchCommitment::new(
    //     vec![],
    //     rollup_last_leaf_index,
    //     genesis_root_hash,
    //     vec![],
    //     vec![],
    //     base_system_contracts_hashes.bootloader,
    //     base_system_contracts_hashes.default_aa,
    // );

    save_genesis_l1_batch_metadata(
        &mut transaction,
        &block_commitment,
        genesis_root_hash,
        rollup_last_leaf_index,
    )
    .await;
    vlog::info!("operations_schema_genesis is complete");

    transaction.commit().await;

    // We need to `println` this value because it will be used to initialize the smart contract.
    println!("CONTRACTS_GENESIS_ROOT={:?}", genesis_root_hash);
    println!(
        "CONTRACTS_GENESIS_BLOCK_COMMITMENT={:?}",
        block_commitment.hash().commitment
    );
    println!(
        "CONTRACTS_GENESIS_ROLLUP_LEAF_INDEX={}",
        rollup_last_leaf_index
    );
    println!(
        "CHAIN_SEQUENCER_BOOTLOADER_HASH={:?}",
        base_system_contracts_hashes.bootloader
    );
    println!(
        "CHAIN_SEQUENCER_DEFAULT_AA_HASH={:?}",
        base_system_contracts_hashes.default_aa
    );

    genesis_root_hash
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
        first_validator_address,
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
        base_fee_per_gas: 0,
        l1_gas_price: 0,
        l2_fair_gas_price: 0,
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
        .insert_l1_batch(&genesis_l1_batch_header, &[], BlockGasCount::default())
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

    add_eth_token(&mut transaction).await;

    transaction.commit().await;
}

pub(crate) async fn save_genesis_l1_batch_metadata(
    storage: &mut StorageProcessor<'_>,
    commitment: &L1BatchCommitment,
    genesis_root_hash: H256,
    rollup_last_leaf_index: u64,
) {
    let commitment_hash = commitment.hash();

    let metadata = L1BatchMetadata {
        root_hash: genesis_root_hash,
        rollup_last_leaf_index,
        merkle_root_hash: genesis_root_hash,
        initial_writes_compressed: vec![],
        repeated_writes_compressed: vec![],
        commitment: commitment_hash.commitment,
        l2_l1_messages_compressed: vec![],
        l2_l1_merkle_root: Default::default(),
        block_meta_params: commitment.meta_parameters(),
        aux_data_hash: commitment_hash.aux_output,
        meta_parameters_hash: commitment_hash.meta_parameters,
        pass_through_data_hash: commitment_hash.pass_through_data,
    };
    storage
        .blocks_dal()
        .save_genesis_l1_batch_metadata(&metadata)
        .await;
}
