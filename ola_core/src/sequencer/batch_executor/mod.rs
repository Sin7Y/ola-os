use std::path::Path;
use std::{fmt, time::Instant};

use async_trait::async_trait;
use ola_config::sequencer::load_network_config;
use ola_dal::connection::ConnectionPool;
use ola_state::rocksdb::RocksdbStorage;
use ola_types::log::{LogQuery, StorageLogQuery};
use ola_types::{ExecuteTransactionCommon, Transaction};
use ola_vm::errors::VmRevertReason;
use ola_vm::{
    errors::TxRevertReason,
    vm::{VmBlockResult, VmExecutionResult, VmPartialExecutionResult, VmTxExecutionResult},
};
use olavm_core::state::error::StateError;
use olavm_core::types::storage::field_arr_to_u8_arr;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use super::{io::L1BatchParams, types::ExecutionMetricsForCriteria};

use ola_types::tx::tx_execution_info::TxExecutionStatus;
use zk_vm::{BlockInfo, TxInfo, VmManager as OlaVmManager};

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

    #[olaos_logs::instrument(skip(self))]
    pub(super) async fn execute_tx(
        &self,
        tx: Transaction,
        tx_index_in_l1_batch: u32,
    ) -> TxExecutionResult {
        let (response_sender, response_receiver) = oneshot::channel();
        self.commands
            .send(Command::ExecuteTx(
                Box::new(tx),
                tx_index_in_l1_batch,
                response_sender,
            ))
            .await
            .unwrap();

        let res = response_receiver.await;
        olaos_logs::info!("receive result");
        if res.is_err() {
            olaos_logs::error!("return err:{:?}", res);
            panic!("ret err:{:?}", res)
        }
        res.unwrap()
    }

    #[olaos_logs::instrument(skip(self))]
    pub(super) async fn finish_batch(self, tx_index_in_l1_batch: u32) -> VmBlockResult {
        let (response_sender, response_receiver) = oneshot::channel();
        self.commands
            .send(Command::FinishBatch(tx_index_in_l1_batch, response_sender))
            .await
            .unwrap();
        let _start = Instant::now();
        let resp = response_receiver.await.unwrap();
        olaos_logs::info!("receive resp");
        self.handle.await.unwrap();
        resp
    }
}

#[derive(Debug)]
pub(super) enum Command {
    ExecuteTx(Box<Transaction>, u32, oneshot::Sender<TxExecutionResult>),
    FinishBatch(u32, oneshot::Sender<VmBlockResult>),
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

    #[allow(unused)]
    fn init_batch_mock(&self, l1_batch_params: L1BatchParams) -> BatchExecutorHandle {
        let secondary_storage = RocksdbStorage::new(self.sequencer_db_path.as_ref());

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
    #[olaos_logs::instrument(skip(self, l1_batch_params), fields(block_number = l1_batch_params.block_number()))]
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
    #[olaos_logs::instrument(skip_all)]
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

        let network_config = load_network_config().expect("failed to load network config");

        let block_info = BlockInfo {
            block_number: l1_batch_params.context_mode.block_number(),
            block_timestamp: l1_batch_params.context_mode.timestamp(),
            sequencer_address: l1_batch_params
                .context_mode
                .operator_address()
                .to_fixed_bytes(),
            chain_id: network_config.ola_network_id,
        };

        let mut vm_manager =
            OlaVmManager::new(block_info, merkle_tree_path, secondary_storage_path);

        while let Some(cmd) = self.commands.blocking_recv() {
            match cmd {
                Command::ExecuteTx(tx, tx_index_in_l1_batch, resp) => {
                    let result = self.execute_tx(&mut vm_manager, &tx, tx_index_in_l1_batch);
                    olaos_logs::info!("execute tx {:?} finished, with error {:?}", tx.hash(), result.err());
                    resp.send(result).unwrap();
                }
                Command::FinishBatch(tx_index_in_l1_batch, resp) => {
                    let block_result = self.finish_batch(&mut vm_manager, tx_index_in_l1_batch);
                    olaos_logs::info!(
                        "finish batch finished, tx_index_in_l1_batch {:?}, total log_queries {:?}",
                        tx_index_in_l1_batch,
                        block_result.full_result.storage_log_queries.len()
                    );
                    resp.send(block_result).unwrap();
                    return;
                }
            }
        }
        // Sequencer can exit because of stop signal, so it's OK to exit mid-batch.
        olaos_logs::info!("Sequencer exited with an unfinished batch");
    }

    #[olaos_logs::instrument(skip_all)]
    fn execute_tx(
        &self,
        vm_manager: &mut OlaVmManager,
        tx: &Transaction,
        tx_index_in_l1_batch: u32,
    ) -> TxExecutionResult {
        let hash = tx.hash();

        olaos_logs::info!(
            "execute tx {:?}, index_in_l1_batch {:?}",
            hash,
            tx_index_in_l1_batch
        );

        let calldata = &tx.execute.calldata;
        let result = match &tx.common_data {
            ExecuteTransactionCommon::L2(tx) => {
                let to_u8_32 = |v: &Vec<u8>| {
                    let mut array = [0; 32];
                    array.copy_from_slice(&v[..32]);
                    array
                };

                let r = tx.signature[0..32].to_vec();
                let s = tx.signature[32..64].to_vec();

                let tx_info = TxInfo {
                    version: tx.transaction_type as u32,
                    caller_address: tx.initiator_address.to_fixed_bytes(),
                    calldata: calldata.to_vec(),
                    nonce: tx.nonce.0,
                    signature_r: to_u8_32(&r),
                    signature_s: to_u8_32(&s),
                    tx_hash: hash.to_fixed_bytes(),
                };
                let exec_res = vm_manager.invoke(tx_info);
                if exec_res.is_ok() {
                    let res = exec_res.unwrap();
                    let ret = field_arr_to_u8_arr(&res.trace.ret);
                    let result = TxExecutionResult::Success {
                        tx_result: Box::new(VmTxExecutionResult {
                            status: TxExecutionStatus::Success,
                            result: VmPartialExecutionResult::new(
                                &res.storage_queries,
                                tx_index_in_l1_batch,
                            ),
                            ret,
                            trace: res.trace,
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
                        entrypoint_dry_run_result: Box::default(),
                    };
                    result
                } else {
                    let revert_reason = VmRevertReason::General {
                        msg: exec_res
                            .err()
                            .unwrap_or_else(|| {
                                StateError::VmExecError("vm internal error".to_string())
                            })
                            .to_string(),
                        data: vec![],
                    };
                    let result = TxExecutionResult::RejectedByVm {
                        rejection_reason: TxRevertReason::TxReverted(revert_reason),
                    };
                    result
                }
            }
            _ => panic!("not support now"),
        };
        result
    }

    #[olaos_logs::instrument(skip_all)]
    fn finish_batch(
        &self,
        vm_manager: &mut OlaVmManager,
        tx_index_in_l1_batch: u32,
    ) -> VmBlockResult {
        let res = vm_manager.finish_batch().unwrap();
        let storage_logs: Vec<StorageLogQuery> = res
            .storage_queries
            .iter()
            .map(|log| {
                let mut log_query: LogQuery = log.into();
                log_query.tx_number_in_block = tx_index_in_l1_batch as u16;
                StorageLogQuery {
                    log_query,
                    log_type: log.kind.into(),
                }
            })
            .collect();
        let mut full_result = VmExecutionResult::default();
        full_result.storage_log_queries = storage_logs;
        VmBlockResult {
            full_result,
            block_tip_result: VmPartialExecutionResult::new(
                &res.block_tip_queries,
                tx_index_in_l1_batch,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sequencer::batch_executor::{MainBatchExecutorBuilder, TxExecutionResult};
    use crate::sequencer::io::L1BatchParams;
    use ola_config::database::{DBConfig, MerkleTreeConfig};
    use ola_config::sequencer::SequencerConfig;
    use ola_contracts::{BaseSystemContracts, BaseSystemContractsHashes, SystemContractCode};
    use ola_dal::connection::{ConnectionPool, DbVariant};
    use ola_types::l2::L2Tx;
    use ola_types::protocol_version::{ProtocolVersion, ProtocolVersionId};
    use ola_types::tx::execute::Execute;
    use ola_types::{Transaction, H256, U256};
    use ola_vm::vm_with_bootloader::{BlockContext, BlockContextMode, BlockProperties};
    use olavm_core::crypto::hash::Hasher;
    use olavm_core::crypto::poseidon::PoseidonHasher;
    use olavm_core::program::binary_program::BinaryProgram;
    use olavm_core::state::error::StateError;
    use olavm_core::storage::db::{Database, RocksDB, SequencerColumnFamily};
    use olavm_core::types::merkle_tree::{
        h256_to_tree_key, tree_key_to_h256, tree_key_to_u8_arr, TreeValue,
    };
    use olavm_core::types::storage::{field_arr_to_u8_arr, u8_arr_to_field_arr};
    use olavm_core::types::{Field, GoldilocksField};
    use rocksdb::WriteBatch;
    use std::fs::File;
    use std::io::BufReader;
    use std::path::PathBuf;
    use std::str::FromStr;
    use tracing::log::LevelFilter;

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
        let _version = ProtocolVersion {
            id: ProtocolVersionId::latest(),
            timestamp: 0,
            base_system_contracts_hashes: base_system_contracts_hashes,
            tx: None,
        };

        // mock deploy contract
        let contract_address = contract_addresses.first().unwrap().clone();
        for (file_name, addr) in binary_files.iter().zip(contract_addresses) {
            let sequencer_db = RocksDB::new(
                Database::Sequencer,
                db_config.sequencer_db_path.clone(),
                false,
            );

            let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            path.push("src/sequencer/test_data/");
            path.push(*file_name);
            let _ = manual_deploy_contract(
                &sequencer_db,
                path.to_str().unwrap(),
                &h256_to_tree_key(&addr),
            );
        }
        // pg database
        let pool_builder = ConnectionPool::singleton(DbVariant::Master);
        let sequencer_pool = pool_builder.build().await;
        // let mut storage = sequencer_pool.access_storage_tagged("sequencer").await;
        // let mut transaction = storage.start_transaction().await;

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

        // let l1_batch = L1BatchHeader {
        //     number: L1BatchNumber(block_numner),
        //     is_finished: true,
        //     timestamp,
        //     l1_tx_count: 0 as u16,
        //     l2_tx_count: 1 as u16,
        //     used_contract_hashes: vec![],
        //     base_system_contracts_hashes,
        //     protocol_version: Some(ProtocolVersionId::latest()),
        // };

        let context = BlockContext {
            block_number: block_numner,
            block_timestamp: timestamp,
            operator_address: H256::zero(),
        };

        // let block_context_properties =
        //     BlockContextMode::NewBlock(context.into(), U256::from_dec_str("1234567").unwrap());
        // let initial_bootloader_contents = UpdatesManager::initial_bootloader_memory(
        //     &L1BatchUpdates::new(),
        //     block_context_properties,
        // );

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
        let batch_executor = batch_executor_base.init_batch_mock(l1_batch_params);

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
        l2_tx.common_data.signature = vec![0; 64];
        let tx = Transaction::from(l2_tx);
        let exec_result = batch_executor.execute_tx(tx.clone(), 0).await;
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

    #[ignore]
    #[tokio::test]
    async fn call_ret_test() {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Info)
            .try_init();
        let addr = tree_key_to_h256(&[
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

    #[ignore]
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
