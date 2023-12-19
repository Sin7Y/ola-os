use std::path::Path;
use std::{fmt, time::Instant};

use async_trait::async_trait;
use ola_dal::connection::ConnectionPool;
use ola_state::{rocksdb::RocksdbStorage, storage_view::StorageView};
use ola_types::{ExecuteTransactionCommon, L1BatchNumber, Transaction, U256};
use ola_vm::{
    errors::TxRevertReason,
    vm::{VmBlockResult, VmExecutionResult, VmPartialExecutionResult, VmTxExecutionResult},
};
use olavm_core::state::error::StateError;
use olavm_core::types::merkle_tree::{
    h160_to_tree_key, h256_to_tree_key, u256_to_tree_key, u8_arr_to_tree_key, TreeValue,
};
use olavm_core::types::storage::{field_arr_to_u8_arr, u8_arr_to_field_arr};
use olavm_core::types::{Field, GoldilocksField};
use olavm_core::vm::transaction::TxCtxInfo;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
use web3::signing::Key;

use super::{io::L1BatchParams, types::ExecutionMetricsForCriteria};
use ola_config::database::DBConfig;
use ola_types::tx::tx_execution_info::TxExecutionStatus;
use zk_vm::OlaVM;

#[derive(Debug)]
pub struct BatchExecutorHandle {
    handle: JoinHandle<()>,
    commands: mpsc::Sender<Command>,
}

impl BatchExecutorHandle {
    pub(super) fn new(
        save_call_traces: bool,
        secondary_storage_path: &Path,
        merkle_tree_path: &Path,
        l1_batch_params: L1BatchParams,
    ) -> Self {
        // Since we process `BatchExecutor` commands one-by-one (the next command is never enqueued
        // until a previous command is processed), capacity 1 is enough for the commands channel.
        let (commands_sender, commands_receiver) = mpsc::channel(1);
        let executor = BatchExecutor {
            save_call_traces,
            commands: commands_receiver,
        };

        let db_path = secondary_storage_path.to_str().unwrap().to_string();
        let merkle_tree_path = merkle_tree_path.to_str().unwrap().to_string();
        let handle = tokio::task::spawn_blocking(move || {
            executor.run(db_path, merkle_tree_path, l1_batch_params)
        });
        Self {
            handle,
            commands: commands_sender,
        }
    }

    pub(super) async fn execute_tx(&self, tx: Transaction) -> TxExecutionResult {
        let (response_sender, response_receiver) = oneshot::channel();
        self.commands
            .send(Command::ExecuteTx(Box::new(tx), response_sender))
            .await
            .unwrap();

        let res = response_receiver.await;
        if res.is_err() {
            panic!("ret err:{:?}", res)
        }
        res.unwrap()
    }

    pub(super) async fn finish_batch(self) -> VmBlockResult {
        let (response_sender, response_receiver) = oneshot::channel();
        self.commands
            .send(Command::FinishBatch(response_sender))
            .await
            .unwrap();
        let _start = Instant::now();
        let resp = response_receiver.await.unwrap();
        self.handle.await.unwrap();
        resp
    }
}

#[derive(Debug)]
pub(super) enum Command {
    ExecuteTx(Box<Transaction>, oneshot::Sender<TxExecutionResult>),
    RollbackLastTx(oneshot::Sender<()>),
    FinishBatch(oneshot::Sender<VmBlockResult>),
}

#[derive(Debug, Clone)]
pub(crate) enum TxExecutionResult {
    /// Successful execution of the tx and the block tip dry run.
    Success {
        tx_result: Box<VmTxExecutionResult>,
        tx_metrics: ExecutionMetricsForCriteria,
        entrypoint_dry_run_metrics: ExecutionMetricsForCriteria,
        entrypoint_dry_run_result: Box<VmPartialExecutionResult>,
    },
    /// The VM rejected the tx for some reason.
    RejectedByVm { rejection_reason: TxRevertReason },
    /// Bootloader gas limit is not enough to execute the tx.
    BootloaderOutOfGasForTx,
    /// Bootloader gas limit is enough to run the tx but not enough to execute block tip.
    BootloaderOutOfGasForBlockTip,
}

impl TxExecutionResult {
    /// Returns a revert reason if either transaction was rejected or bootloader ran out of gas.
    pub(super) fn err(&self) -> Option<&TxRevertReason> {
        match self {
            Self::Success { .. } => None,
            Self::RejectedByVm { rejection_reason } => Some(rejection_reason),
            Self::BootloaderOutOfGasForTx | Self::BootloaderOutOfGasForBlockTip { .. } => {
                Some(&TxRevertReason::BootloaderOutOfGas)
            }
        }
    }
}

#[async_trait]
pub trait L1BatchExecutorBuilder: 'static + Send + Sync + fmt::Debug {
    async fn init_batch(&self, l1_batch_params: L1BatchParams) -> BatchExecutorHandle;
}

#[derive(Debug, Clone)]
pub struct MainBatchExecutorBuilder {
    sequencer_db_path: String,
    merkle_db_path: String,
    pool: ConnectionPool,
    save_call_traces: bool,
}

impl MainBatchExecutorBuilder {
    pub fn new(
        sequencer_db_path: String,
        merkle_db_path: String,
        pool: ConnectionPool,
        save_call_traces: bool,
    ) -> Self {
        Self {
            sequencer_db_path,
            merkle_db_path,
            pool,
            save_call_traces,
        }
    }

    async fn init_batch_mock(&self, l1_batch_params: L1BatchParams) -> BatchExecutorHandle {
        let mut secondary_storage = RocksdbStorage::new(self.sequencer_db_path.as_ref());

        let batch_number = l1_batch_params
            .context_mode
            .inner_block_context()
            .context
            .block_number;

        olaos_logs::info!(
            "Secondary storage for batch {batch_number} initialized, size is {}",
            secondary_storage.estimated_map_size()
        );
        BatchExecutorHandle::new(
            self.save_call_traces,
            self.sequencer_db_path.as_ref(),
            self.merkle_db_path.as_ref(),
            l1_batch_params,
        )
    }
}

#[async_trait]
impl L1BatchExecutorBuilder for MainBatchExecutorBuilder {
    async fn init_batch(&self, l1_batch_params: L1BatchParams) -> BatchExecutorHandle {
        let mut secondary_storage = RocksdbStorage::new(self.sequencer_db_path.as_ref());
        let mut conn = self.pool.access_storage_tagged("sequencer").await;
        secondary_storage.update_from_postgres(&mut conn).await;
        drop(conn);

        let batch_number = l1_batch_params
            .context_mode
            .inner_block_context()
            .context
            .block_number;

        olaos_logs::info!(
            "Secondary storage for batch {batch_number} initialized, size is {}",
            secondary_storage.estimated_map_size()
        );
        BatchExecutorHandle::new(
            self.save_call_traces,
            self.sequencer_db_path.as_ref(),
            self.merkle_db_path.as_ref(),
            l1_batch_params,
        )
    }
}

#[derive(Debug)]
pub(super) struct BatchExecutor {
    save_call_traces: bool,
    commands: mpsc::Receiver<Command>,
}

impl BatchExecutor {
    pub(super) fn run(
        mut self,
        secondary_storage_path: String,
        merkle_tree_path: String,
        l1_batch_params: L1BatchParams,
    ) {
        olaos_logs::info!(
            "Starting executing batch #{}",
            l1_batch_params
                .context_mode
                .inner_block_context()
                .context
                .block_number
        );

        // TODO: @pierre init vm begin
        // let mut storage_view = StorageView::new(&secondary_storage);
        // let block_properties = BlockProperties::new(
        //     self.vm_version,
        //     l1_batch_params.properties.default_aa_code_hash,
        // );

        // let mut vm = match self.vm_gas_limit {
        //     Some(vm_gas_limit) => init_vm_with_gas_limit(
        //         self.vm_version,
        //         &mut oracle_tools,
        //         l1_batch_params.context_mode,
        //         &block_properties,
        //         TxExecutionMode::VerifyExecute,
        //         &l1_batch_params.base_system_contracts,
        //         vm_gas_limit,
        //     ),
        //     None => init_vm(
        //         self.vm_version,
        //         &mut oracle_tools,
        //         l1_batch_params.context_mode,
        //         &block_properties,
        //         TxExecutionMode::VerifyExecute,
        //         &l1_batch_params.base_system_contracts,
        //     ),
        // };
        // TODO: need roscksdb path for storage merkle tree

        // TODO: @pierre init vm end
        // secondary_storage.load_factory_dep();
        let tx_ctx = TxCtxInfo {
            block_number: GoldilocksField::from_canonical_u32(
                l1_batch_params
                    .context_mode
                    .inner_block_context()
                    .context
                    .block_number,
            ),
            block_timestamp: GoldilocksField::from_canonical_u64(
                l1_batch_params
                    .context_mode
                    .inner_block_context()
                    .context
                    .block_timestamp,
            ),
            sequencer_address: h256_to_tree_key(
                &l1_batch_params.base_system_contracts.entrypoint.hash,
            ),
            version: GoldilocksField::from_canonical_u64(l1_batch_params.protocol_version as u64),
            chain_id: GoldilocksField::from_canonical_u64(1),
            caller_address: h256_to_tree_key(
                &l1_batch_params.base_system_contracts.default_aa.hash,
            ),
            nonce: GoldilocksField::ZERO,
            signature: Default::default(),
            tx_hash: Default::default(),
        };
        while let Some(cmd) = self.commands.blocking_recv() {
            match cmd {
                Command::ExecuteTx(tx, resp) => {
                    let mut tx_ctx_info = tx_ctx.clone();
                    match tx.common_data {
                        ExecuteTransactionCommon::L2(tx) => {
                            tx_ctx_info.signature = u8_arr_to_tree_key(&tx.signature);
                            tx_ctx_info.nonce = GoldilocksField::from_canonical_u32(tx.nonce.0);
                        }
                        _ => panic!("not support now"),
                    }
                    let mut vm = OlaVM::new(
                        merkle_tree_path.as_ref(),
                        secondary_storage_path.as_ref(),
                        tx_ctx_info,
                    );
                    // FIXME: @pierre
                    let address = h256_to_tree_key(&tx.execute.contract_address);
                    let calldata = u8_arr_to_field_arr(&tx.execute.calldata);
                    let exec_res = self.execute_tx(&mut vm, address, calldata);
                    if exec_res.is_ok() {
                        let tx_trace = vm.ola_state.gen_tx_trace();
                        let ret = field_arr_to_u8_arr(&tx_trace.ret);
                        let result = TxExecutionResult::Success {
                            tx_result: Box::new(VmTxExecutionResult {
                                status: TxExecutionStatus::Success,
                                result: Default::default(),
                                ret,
                                trace: tx_trace,
                                call_traces: vec![],
                                gas_refunded: 0,
                                operator_suggested_refund: 0,
                            }),
                            tx_metrics: ExecutionMetricsForCriteria {
                                execution_metrics: Default::default(),
                            },
                            entrypoint_dry_run_metrics: ExecutionMetricsForCriteria {
                                execution_metrics: Default::default(),
                            },
                            entrypoint_dry_run_result: Box::new(Default::default()),
                        };
                        resp.send(result).unwrap();
                    } else {
                        println!("exec tx err: {:?}", exec_res);
                        let result = TxExecutionResult::BootloaderOutOfGasForBlockTip;
                        resp.send(result).unwrap();
                    }
                }
                Command::RollbackLastTx(resp) => {
                    // FIXME: @pierre
                    // self.rollback_last_tx(&mut vm);
                    resp.send(()).unwrap();
                }
                Command::FinishBatch(resp) => {
                    // resp.send(self.finish_batch(&mut vm)).unwrap();
                    return;
                }
            }
        }
        // Sequencer can exit because of stop signal, so it's OK to exit mid-batch.
        olaos_logs::info!("Sequencer exited with an unfinished batch");
    }

    fn finish_batch(&self, _vm: &mut OlaVM) -> VmBlockResult {
        // FIXME: @pierre
        // vm.execute_till_block_end(BootloaderJobType::BlockPostprocessing)
        VmBlockResult {
            full_result: VmExecutionResult::default(),
            block_tip_result: VmPartialExecutionResult::default(),
        }
    }

    fn execute_tx(
        &self,
        vm: &mut OlaVM,
        caller: TreeValue,
        calldata: Vec<GoldilocksField>,
    ) -> Result<(), StateError> {
        vm.execute_tx(caller, caller, calldata, false)
    }
}

#[cfg(test)]
mod tests {
    use crate::sequencer::batch_executor::{
        L1BatchExecutorBuilder, MainBatchExecutorBuilder, TxExecutionResult,
    };
    use crate::sequencer::io::common::l1_batch_params;
    use crate::sequencer::io::L1BatchParams;
    use crate::sequencer::updates::l1_batch_updates::L1BatchUpdates;
    use crate::sequencer::updates::UpdatesManager;
    use ola_config::database::{load_db_config, DBConfig, MerkleTreeConfig};
    use ola_config::sequencer::SequencerConfig;
    use ola_contracts::{BaseSystemContracts, BaseSystemContractsHashes, SystemContractCode};
    use ola_dal::connection::{ConnectionPool, DbVariant};
    use ola_types::block::L1BatchHeader;
    use ola_types::l2::L2Tx;
    use ola_types::protocol_version::{ProtocolVersion, ProtocolVersionId};
    use ola_types::tx::execute::Execute;
    use ola_types::{L1BatchNumber, Transaction, H256, U256};
    use ola_vm::vm_with_bootloader::{BlockContext, BlockContextMode, BlockProperties};
    use olavm_core::crypto::hash::Hasher;
    use olavm_core::crypto::poseidon::PoseidonHasher;
    use olavm_core::program::binary_program::BinaryProgram;
    use olavm_core::state::error::StateError;
    use olavm_core::storage::db::{
        Database, RocksDB, SequencerColumnFamily as VM_SequencerColumnFamily, SequencerColumnFamily,
    };
    use olavm_core::types::merkle_tree::{
        h160_to_tree_key, h256_to_tree_key, tree_key_to_h256, tree_key_to_u8_arr, u256_to_tree_key,
        TreeValue,
    };
    use olavm_core::types::storage::{field_arr_to_u8_arr, u8_arr_to_field_arr};
    use olavm_core::types::{Field, GoldilocksField};
    use rocksdb::WriteBatch;
    use std::fs::File;
    use std::io::BufReader;
    use std::path::PathBuf;
    use std::str::FromStr;
    use tracing::log::LevelFilter;
    use web3::ethabi::Address;
    use web3::types::H160;

    pub fn save_contract_map(
        db: &RocksDB,
        contract_addr: &TreeValue,
        code_hash: &Vec<u8>,
    ) -> Result<(), StateError> {
        let mut batch = WriteBatch::default();
        let cf = db.cf_sequencer_handle(SequencerColumnFamily::ContractMap);
        let code_key = tree_key_to_u8_arr(contract_addr);
        batch.put_cf(cf, &code_key, code_hash);

        db.write(batch).map_err(StateError::StorageIoError)
    }

    pub fn save_program(
        db: &RocksDB,
        code_hash: &Vec<u8>,
        code: &Vec<u8>,
    ) -> Result<(), StateError> {
        let mut batch = WriteBatch::default();
        let cf = db.cf_sequencer_handle(SequencerColumnFamily::FactoryDeps);

        batch.put_cf(cf, code_hash, code);
        db.write(batch).map_err(StateError::StorageIoError)
    }

    pub fn manual_deploy_contract(
        db: &RocksDB,
        contract_path: &str,
        addr: &TreeValue,
    ) -> Result<TreeValue, StateError> {
        let file = File::open(contract_path).unwrap();
        let reader = BufReader::new(file);
        let program: BinaryProgram = serde_json::from_reader(reader).unwrap();
        let instructions = program.bytecode.split("\n");

        let code: Vec<_> = instructions
            .map(|e| GoldilocksField::from_canonical_u64(u64::from_str_radix(&e[2..], 16).unwrap()))
            .collect();

        let hasher = PoseidonHasher;
        let code_hash = hasher.hash_bytes(&code);
        save_program(
            &db,
            &tree_key_to_u8_arr(&code_hash),
            &serde_json::to_string_pretty(&program)
                .unwrap()
                .as_bytes()
                .to_vec(),
        )?;

        save_contract_map(&db, addr, &tree_key_to_u8_arr(&code_hash))?;

        Ok(code_hash)
    }

    fn default_sequencer_config() -> SequencerConfig {
        SequencerConfig {
            miniblock_seal_queue_capacity: 10,
            miniblock_commit_deadline_ms: 1000,
            block_commit_deadline_ms: 2500,
            reject_tx_at_geometry_percentage: 0.0,
            close_block_at_geometry_percentage: 0.0,
            fee_account_addr: H256::from_str(
                "0x0100038581be3d0e201b3cc45d151ef5cc59eb3a0f146ad44f0f72abf00b0000",
            )
            .unwrap(),
            entrypoint_hash: H256::from_str(
                "0x0100038581be3d0e201b3cc45d151ef5cc59eb3a0f146ad44f0f72abf00b594c",
            )
            .unwrap(),
            default_aa_hash: H256::from_str(
                "0x0100038dc66b69be75ec31653c64cb931678299b9b659472772b2550b703f41c",
            )
            .unwrap(),
            transaction_slots: 250,
            save_call_traces: true,
        }
    }

    async fn batch_execute_tx(
        binary_files: Vec<&str>,
        contract_addresses: Vec<H256>,
        calldata: Vec<u8>,
        sequencer_db_path: String,
        merkle_tree_path: String,
        backup_path: String,
    ) {
        let db_config = DBConfig {
            statement_timeout_sec: Some(300),
            sequencer_db_path,
            merkle_tree: MerkleTreeConfig {
                path: merkle_tree_path,
                backup_path,
                mode: Default::default(),
                multi_get_chunk_size: 1000,
                block_cache_size_mb: 128,
                max_l1_batches_per_iter: 50,
            },
            backup_count: 5,
            backup_interval_ms: 60000,
        };

        let timestamp = 1702458919000;
        let block_numner = 1;
        let sequencer_config = default_sequencer_config();

        let base_system_contracts_hashes = BaseSystemContractsHashes {
            entrypoint: sequencer_config.entrypoint_hash,
            default_aa: sequencer_config.default_aa_hash,
        };
        let version = ProtocolVersion {
            id: ProtocolVersionId::latest(),
            timestamp: 0,
            base_system_contracts_hashes: base_system_contracts_hashes,
            tx: None,
        };

        // mock deploy contract
        let contract_address = contract_addresses.first().unwrap().clone();
        for (file_name, addr) in binary_files.iter().zip(contract_addresses) {
            let mut sequencer_db = RocksDB::new(
                Database::Sequencer,
                db_config.sequencer_db_path.clone(),
                false,
            );

            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("src/sequencer/test_data/");
            path.push(*file_name);
            manual_deploy_contract(
                &sequencer_db,
                path.to_str().unwrap(),
                &h256_to_tree_key(&addr),
            );
        }
        // pg database
        let pool_builder = ConnectionPool::singleton(DbVariant::Master);
        let sequencer_pool = pool_builder.build().await;
        let mut storage = sequencer_pool.access_storage_tagged("sequencer").await;
        let mut transaction = storage.start_transaction().await;

        // init version
        // transaction
        //     .protocol_versions_dal()
        //     .save_protocol_version(version)
        //     .await;

        // init l1_batch
        // transaction
        //     .blocks_dal()
        //     .insert_l1_batch(&l1_batch, &initial_bootloader_contents)
        //     .await;

        let l1_batch = L1BatchHeader {
            number: L1BatchNumber(block_numner),
            is_finished: true,
            timestamp,
            l1_tx_count: 0 as u16,
            l2_tx_count: 1 as u16,
            used_contract_hashes: vec![],
            base_system_contracts_hashes,
            protocol_version: Some(ProtocolVersionId::latest()),
        };

        let context = BlockContext {
            block_number: block_numner,
            block_timestamp: timestamp,
            operator_address: H256::zero(),
        };

        let block_context_properties =
            BlockContextMode::NewBlock(context.into(), U256::from_dec_str("1234567").unwrap());
        let initial_bootloader_contents = UpdatesManager::initial_bootloader_memory(
            &L1BatchUpdates::new(),
            block_context_properties,
        );

        let batch_executor_base = MainBatchExecutorBuilder::new(
            db_config.sequencer_db_path.clone(),
            db_config.merkle_tree.path.clone(),
            sequencer_pool.clone(),
            sequencer_config.save_call_traces,
        );

        let l1_batch_params = L1BatchParams {
            context_mode: BlockContextMode::NewBlock(
                context.into(),
                U256::from_dec_str("1234567").unwrap(),
            ),
            properties: BlockProperties {
                default_aa_code_hash: Default::default(),
            },
            base_system_contracts: BaseSystemContracts {
                entrypoint: SystemContractCode {
                    code: vec![],
                    hash: Default::default(),
                },
                default_aa: SystemContractCode {
                    code: vec![],
                    hash: Default::default(),
                },
            },
            protocol_version: ProtocolVersionId::latest(),
        };
        let batch_executor = batch_executor_base.init_batch_mock(l1_batch_params).await;

        //construct tx
        let mut l2_tx = L2Tx {
            execute: Execute {
                contract_address,
                calldata,
                factory_deps: None,
            },
            common_data: Default::default(),
            received_timestamp_ms: 0,
        };
        l2_tx.common_data.signature = vec![0; 32];
        let tx = Transaction::from(l2_tx);
        let exec_result = batch_executor.execute_tx(tx.clone()).await;
        match exec_result {
            TxExecutionResult::Success { tx_result, .. } => {
                println!("tx ret:{:?}", u8_arr_to_field_arr(&tx_result.ret))
            }
            TxExecutionResult::RejectedByVm { .. } => {}
            TxExecutionResult::BootloaderOutOfGasForTx => {
                println!("tx exec res:{:?}", exec_result);
            }
            TxExecutionResult::BootloaderOutOfGasForBlockTip => {}
        }
    }
    #[tokio::test]
    async fn call_ret_test() {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Info)
            .try_init();
        let mut addr = tree_key_to_h256(&[
            GoldilocksField::ONE,
            GoldilocksField::ONE,
            GoldilocksField::ZERO,
            GoldilocksField::ONE,
        ]);

        let call_data = [5, 11, 2, 2062500454];

        let calldata = call_data
            .iter()
            .map(|e| GoldilocksField::from_canonical_u64(*e))
            .collect();
        batch_execute_tx(
            vec!["call_ret.json"],
            vec![addr],
            field_arr_to_u8_arr(&calldata),
            "./db/call_ret/tree".to_string(),
            "./db/call_ret/merkle_tree".to_string(),
            "./db/call_ret/backups".to_string(),
        )
        .await;
    }

    #[tokio::test]
    async fn sccall_test() {
        let addr_caller = tree_key_to_h256(&[
            GoldilocksField::ONE,
            GoldilocksField::ONE,
            GoldilocksField::ZERO,
            GoldilocksField::ONE,
        ]);

        let addr_callee = tree_key_to_h256(&[
            GoldilocksField::ONE,
            GoldilocksField::ZERO,
            GoldilocksField::ONE,
            GoldilocksField::ZERO,
        ]);
        let call_data = [1, 0, 1, 0, 4, 645225708];
        let calldata = call_data
            .iter()
            .map(|e| GoldilocksField::from_canonical_u64(*e))
            .collect();
        batch_execute_tx(
            vec!["sccall_caller.json", "sccall_callee.json"],
            vec![addr_caller, addr_callee],
            field_arr_to_u8_arr(&calldata),
            "./db/sccall/tree".to_string(),
            "./db/sccall/merkle_tree".to_string(),
            "./db/sccall/backups".to_string(),
        )
        .await;
    }
}
