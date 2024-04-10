use std::io::Write;

use futures::FutureExt;
use jsonrpsee::{
    core::{server::SubscriptionMessage, SubscriptionResult},
    server::IdProvider,
    types::{error::ErrorCode, ErrorObject, SubscriptionId},
    PendingSubscriptionSink, SendTimeoutError, SubscriptionSink,
};
use ola_contracts::BaseSystemContractsHashes;
use ola_dal::{connection::ConnectionPool, StorageProcessor};
use ola_types::{
    block::L1BatchHeader,
    commitment::{L1BatchMetaParameters, L1BatchMetadata, L1BatchWithMetadata},
    proofs::L1BatchProofForL1,
    protocol_version::ProtocolVersionId,
    prove_batches::ProveBatches,
    Address, L1BatchNumber, MiniblockNumber, H256,
};
use ola_web3_decl::{
    namespaces::eth::EthPubSubServer,
    types::{L1BatchProofForVerify, PubSubFilter, PubSubResult},
};
use olaos_prover_fri_types::{FriProofWrapper, OlaBaseLayerProof};
use tokio::{
    sync::{broadcast, mpsc, watch},
    task::JoinHandle,
    time::{interval, Duration},
};
use web3::types::H128;

const BROADCAST_CHANNEL_CAPACITY: usize = 1024;
const SUBSCRIPTION_SINK_SEND_TIMEOUT: Duration = Duration::from_secs(10);
pub const EVENT_TOPIC_NUMBER_LIMIT: usize = 4;

#[derive(Debug, Clone, Copy)]
pub struct EthSubscriptionIdProvider;

impl IdProvider for EthSubscriptionIdProvider {
    fn next_id(&self) -> SubscriptionId<'static> {
        let id = H128::random();
        format!("0x{}", hex::encode(id.0)).into()
    }
}

/// Events emitted by the subscription logic. Only used in WebSocket server tests so far.
#[derive(Debug)]
pub(super) enum PubSubEvent {
    Subscribed(SubscriptionType),
    NotifyIterationFinished(SubscriptionType),
    MiniblockAdvanced(SubscriptionType, MiniblockNumber),
    L1BatchVerifiedAdvanced(SubscriptionType, L1BatchNumber),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum SubscriptionType {
    Blocks,
    Txs,
    Logs,
    L1BatchProofs,
}

/// Manager of notifications for a certain type of subscriptions.
#[derive(Debug)]
struct PubSubNotifier {
    sender: broadcast::Sender<Vec<PubSubResult>>,
    connection_pool: ConnectionPool,
    polling_interval: Duration,
    events_sender: Option<mpsc::UnboundedSender<PubSubEvent>>,
}

impl PubSubNotifier {
    // async fn get_starting_miniblock_number(&self) -> anyhow::Result<MiniblockNumber> {
    //     let mut storage = self
    //         .connection_pool
    //         .access_storage_tagged("api")
    //         .await
    //         .context("access_storage_tagged")?;
    //     let sealed_miniblock_number = storage
    //         .blocks_dal()
    //         .get_sealed_miniblock_number()
    //         .await
    //         .context("get_sealed_miniblock_number()")?;
    //     Ok(match sealed_miniblock_number {
    //         Some(number) => number,
    //         None => {
    //             // We don't have miniblocks in the storage yet. Use the snapshot miniblock number instead.
    //             let start_info = BlockStartInfo::new(&mut storage).await?;
    //             MiniblockNumber(start_info.first_miniblock.saturating_sub(1))
    //         }
    //     })
    // }

    fn emit_event(&self, event: PubSubEvent) {
        if let Some(sender) = &self.events_sender {
            sender.send(event).ok();
        }
    }
}

impl PubSubNotifier {
    // async fn notify_blocks(self, stop_receiver: watch::Receiver<bool>) -> anyhow::Result<()> {
    //     let mut last_block_number = self.get_starting_miniblock_number().await?;
    //     let mut timer = Interval(self.polling_interval);
    //     loop {
    //         if *stop_receiver.borrow() {
    //             tracing::info!("Stop signal received, pubsub_block_notifier is shutting down");
    //             break;
    //         }
    //         timer.tick().await;

    //         let db_latency = PUB_SUB_METRICS.db_poll_latency[&SubscriptionType::Blocks].start();
    //         let new_blocks = self.new_blocks(last_block_number).await?;
    //         db_latency.observe();

    //         if let Some(last_block) = new_blocks.last() {
    //             last_block_number = MiniblockNumber(last_block.number.unwrap().as_u32());
    //             let new_blocks = new_blocks.into_iter().map(PubSubResult::Header).collect();
    //             self.send_pub_sub_results(new_blocks, SubscriptionType::Blocks);
    //             self.emit_event(PubSubEvent::MiniblockAdvanced(
    //                 SubscriptionType::Blocks,
    //                 last_block_number,
    //             ));
    //         }
    //         self.emit_event(PubSubEvent::NotifyIterationFinished(
    //             SubscriptionType::Blocks,
    //         ));
    //     }
    // Ok(())
    // }

    fn send_pub_sub_results(&self, results: Vec<PubSubResult>, sub_type: SubscriptionType) {
        // Errors only on 0 receivers, but we want to go on if we have 0 subscribers so ignore the error.
        self.sender.send(results).ok();
        // PUB_SUB_METRICS.broadcast_channel_len[&sub_type].set(self.sender.len());
    }

    // async fn new_blocks(
    //     &self,
    //     last_block_number: MiniblockNumber,
    // ) -> anyhow::Result<Vec<BlockHeader>> {
    //     self.connection_pool
    //         .access_storage_tagged("api")
    //         .await
    //         .context("access_storage_tagged")?
    //         .blocks_web3_dal()
    //         .get_block_headers_after(last_block_number)
    //         .await
    //         .with_context(|| format!("get_block_headers_after({last_block_number})"))
    // }

    // async fn notify_txs(self, stop_receiver: watch::Receiver<bool>) -> anyhow::Result<()> {
    //     let mut last_time = chrono::Utc::now().naive_utc();
    //     let mut timer = interval(self.polling_interval);
    //     loop {
    //         if *stop_receiver.borrow() {
    //             tracing::info!("Stop signal received, pubsub_tx_notifier is shutting down");
    //             break;
    //         }
    //         timer.tick().await;

    //         // let db_latency = PUB_SUB_METRICS.db_poll_latency[&SubscriptionType::Txs].start();
    //         let (new_txs, new_last_time) = self.new_txs(last_time).await?;
    //         // db_latency.observe();

    //         if let Some(new_last_time) = new_last_time {
    //             last_time = new_last_time;
    //             let new_txs = new_txs.into_iter().map(PubSubResult::TxHash).collect();
    //             self.send_pub_sub_results(new_txs, SubscriptionType::Txs);
    //         }
    //         self.emit_event(PubSubEvent::NotifyIterationFinished(SubscriptionType::Txs));
    //     }
    // Ok(())
    // }

    // async fn new_txs(
    //     &self,
    //     last_time: chrono::NaiveDateTime,
    // ) -> anyhow::Result<(Vec<H256>, Option<chrono::NaiveDateTime>)> {
    //     self.connection_pool
    //         .access_storage_tagged("api")
    //         .await
    //         .context("access_storage_tagged")?
    //         .transactions_web3_dal()
    //         .get_pending_txs_hashes_after(last_time, None)
    //         .await
    //         .context("get_pending_txs_hashes_after()")
    // }

    // async fn notify_logs(self, stop_receiver: watch::Receiver<bool>) -> anyhow::Result<()> {
    //     let mut last_block_number = self.get_starting_miniblock_number().await?;

    //     let mut timer = interval(self.polling_interval);
    //     loop {
    //         if *stop_receiver.borrow() {
    //             tracing::info!("Stop signal received, pubsub_logs_notifier is shutting down");
    //             break;
    //         }
    //         timer.tick().await;

    //         let db_latency = PUB_SUB_METRICS.db_poll_latency[&SubscriptionType::Logs].start();
    //         let new_logs = self.new_logs(last_block_number).await?;
    //         db_latency.observe();

    //         if let Some(last_log) = new_logs.last() {
    //             last_block_number = MiniblockNumber(last_log.block_number.unwrap().as_u32());
    //             let new_logs = new_logs.into_iter().map(PubSubResult::Log).collect();
    //             self.send_pub_sub_results(new_logs, SubscriptionType::Logs);
    //             self.emit_event(PubSubEvent::MiniblockAdvanced(
    //                 SubscriptionType::Logs,
    //                 last_block_number,
    //             ));
    //         }
    //         self.emit_event(PubSubEvent::NotifyIterationFinished(SubscriptionType::Logs));
    //     }
    // Ok(())
    // }

    // async fn new_logs(&self, last_block_number: MiniblockNumber) -> anyhow::Result<Vec<Log>> {
    //     self.connection_pool
    //         .access_storage_tagged("api")
    //         .await
    //         .context("access_storage_tagged")?
    //         .events_web3_dal()
    //         .get_all_logs(last_block_number)
    //         .await
    //         .context("events_web3_dal().get_all_logs()")
    // }

    async fn notify_l1_batch_proofs(
        self,
        stop_receiver: watch::Receiver<bool>,
    ) -> anyhow::Result<()> {
        let mut timer = interval(self.polling_interval);
        loop {
            if *stop_receiver.borrow() {
                tracing::info!("Stop signal received, pubsub_logs_notifier is shutting down");
                break;
            }
            timer.tick().await;

            let new_proof = self.new_l1_batch_proofs().await?;

            if let Some(new_proof) = new_proof {
                let proof_bytes = bincode::serialize(&new_proof)?;
                let proof = L1BatchProofForVerify {
                    l1_batch_number: new_proof.prev_l1_batch.header.number + 1,
                    prove_batches_data: proof_bytes,
                };
                let proof = PubSubResult::L1BatchProof(proof);
                let last_l1_batch_number = new_proof.l1_batches.last().unwrap().header.number;
                self.send_pub_sub_results(vec![proof], SubscriptionType::L1BatchProofs);
                self.emit_event(PubSubEvent::L1BatchVerifiedAdvanced(
                    SubscriptionType::L1BatchProofs,
                    last_l1_batch_number,
                ));
            }
        }
        Ok(())
    }

    async fn new_l1_batch_proofs(&self) -> anyhow::Result<Option<ProveBatches>> {
        let mut storage = self.connection_pool.access_storage_tagged("api").await;
        // let blob_store = self.blob_store.clone().expect("blob_store not specified");
        // TODO:
        // Self::load_proof_for_offchain_verify(&mut storage, &*blob_store).await
        Self::load_proof_for_offchain_verify_mock(&mut storage).await
    }

    async fn load_proof_for_offchain_verify_mock(
        storage: &mut StorageProcessor<'_>,
        // blob_store: &dyn ObjectStore,
    ) -> anyhow::Result<Option<ProveBatches>> {
        olaos_logs::info!("start read mock proof bin file");

        let ola_home = std::env::var("OLAOS_HOME").unwrap_or_else(|_| ".".into());
        let bin_path = format!("{}/ola_core/src/api_server/web3/ProveBatches.bin", ola_home);
        let prove_batches_data =
            std::fs::read(bin_path).expect("failed to read ProveBatches.bin file");
        let prove_batches: ProveBatches =
            bincode::deserialize(&prove_batches_data).expect("failed to deserialize ProveBatches");
        let proof = prove_batches.proofs.first().unwrap().to_owned();
        // let proof: FriProofWrapper = bincode::deserialize(&proof.proof).unwrap();
        let proof: FriProofWrapper =
            serde_json::from_slice(&proof.proof).expect("faile to deserialize from slice");
        match proof {
            FriProofWrapper::Base(ola_proof) => {
                olaos_logs::info!(
                    "Mock proof bitwise challenge: {:?}",
                    ola_proof.ola_stark.bitwise_stark.get_compress_challenge()
                );
            }
        }
        olaos_logs::info!("read mock proof bin file successfully");

        // let proof_path = format!(
        //     "{}/ola_core/src/api_server/web3/proof.bin",
        //     ola_home
        // );
        // let proof_data = std::fs::read(proof_path).unwrap();
        // // // let proof: OlaBaseLayerProof = bincode::deserialize(&proof_data).unwrap();
        // let proof: OlaBaseLayerProof = serde_json::from_slice(&proof_data).unwrap();
        // let proof_wrapper = FriProofWrapper::Base(proof);
        // let data = serde_json::to_string(&proof_wrapper).unwrap();
        // // let data = bincode::serialize(&proof_wrapper).unwrap();
        // let l1_batch_proof = L1BatchProofForL1 { proof: data.as_bytes().to_vec() };

        // let header = L1BatchHeader {
        //     number: L1BatchNumber(0),
        //     is_finished: true,
        //     timestamp: 0,
        //     fee_account_address: Address::default(),
        //     l1_tx_count: 0,
        //     l2_tx_count: 0,
        //     l2_to_l1_logs: vec![],
        //     l2_to_l1_messages: vec![],
        //     priority_ops_onchain_data: vec![],
        //     used_contract_hashes: vec![],
        //     base_system_contracts_hashes: BaseSystemContractsHashes::default(),
        //     protocol_version: Some(ProtocolVersionId::latest()),
        // };
        // let meta_data = L1BatchMetadata {
        //     root_hash: H256::default(),
        //     rollup_last_leaf_index: 0,
        //     merkle_root_hash: H256::default(),
        //     initial_writes_compressed: vec![],
        //     repeated_writes_compressed: vec![],
        //     commitment: H256::default(),
        //     l2_l1_messages_compressed: vec![],
        //     l2_l1_merkle_root: H256::default(),
        //     block_meta_params: L1BatchMetaParameters {
        //         bootloader_code_hash: H256::default(),
        //         default_aa_code_hash: H256::default(),
        //     },
        //     aux_data_hash: H256::default(),
        //     meta_parameters_hash: H256::default(),
        //     pass_through_data_hash: H256::default(),
        //     state_diffs_compressed: vec![],
        //     events_queue_commitment: None,
        // };
        // let batch_with_meta_data = L1BatchWithMetadata {
        //     header: header,
        //     metadata: meta_data,
        //     factory_deps: vec![],
        // };
        // let prove_batches = ProveBatches {
        //     prev_l1_batch: batch_with_meta_data.clone(),
        //     l1_batches: vec![batch_with_meta_data],
        //     proofs: vec![l1_batch_proof],
        //     should_verify: true,
        // };
        // let mut proof_file = std::fs::File::create("./ProveBatches.bin").unwrap();
        // let proof_batch_data = bincode::serialize(&prove_batches).unwrap();
        // proof_file.write_all(&proof_batch_data).unwrap();
        // writeln!(proof_file).unwrap();
        Ok(Some(prove_batches))
    }
}

pub(super) struct EthSubscribe {
    blocks: broadcast::Sender<Vec<PubSubResult>>,
    transactions: broadcast::Sender<Vec<PubSubResult>>,
    logs: broadcast::Sender<Vec<PubSubResult>>,
    l1_batch_proofs: broadcast::Sender<Vec<PubSubResult>>,
    events_sender: Option<mpsc::UnboundedSender<PubSubEvent>>,
}

impl EthSubscribe {
    pub fn new() -> Self {
        let (blocks, _) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
        let (transactions, _) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
        let (logs, _) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);
        let (l1_batch_proofs, _) = broadcast::channel(BROADCAST_CHANNEL_CAPACITY);

        Self {
            blocks,
            transactions,
            logs,
            l1_batch_proofs,
            events_sender: None,
        }
    }

    pub fn set_events_sender(&mut self, sender: mpsc::UnboundedSender<PubSubEvent>) {
        self.events_sender = Some(sender);
    }

    async fn reject(sink: PendingSubscriptionSink) {
        sink.reject(ErrorObject::borrowed(
            ErrorCode::InvalidParams.code(),
            &"Rejecting subscription - invalid parameters provided.",
            None,
        ))
        .await;
    }

    async fn run_subscriber(
        sink: SubscriptionSink,
        subscription_type: SubscriptionType,
        mut receiver: broadcast::Receiver<Vec<PubSubResult>>,
        filter: Option<PubSubFilter>,
    ) {
        // let _guard = PUB_SUB_METRICS.active_subscribers[&subscription_type].inc_guard(1);
        // let lifetime_latency = PUB_SUB_METRICS.subscriber_lifetime[&subscription_type].start();
        let closed = sink.closed().fuse();
        tokio::pin!(closed);

        loop {
            tokio::select! {
                new_items_result = receiver.recv() => {
                    let new_items = match new_items_result {
                        Ok(items) => items,
                        Err(broadcast::error::RecvError::Closed) => {
                            // The broadcast channel has closed because the notifier task is shut down.
                            // This is fine; we should just stop this task.
                            olaos_logs::error!("subscription_type {:?} closed", subscription_type);
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(message_count)) => {
                            olaos_logs::error!("skipped_broadcast_message {:?} count {:?}", subscription_type, message_count);
                            break;
                        }
                    };

                    let handle_result = Self::handle_new_items(
                        &sink,
                        subscription_type,
                        new_items,
                        filter.as_ref()
                    )
                    .await;
                    if handle_result.is_err() {
                        olaos_logs::error!("subscriber_send_timeouts {:?} error {:?}", subscription_type, handle_result);
                        break;
                    }
                }
                _ = &mut closed => {
                    break;
                }
            }
        }
        olaos_logs::info!("run_subscriber {:?} finished", subscription_type);
    }

    async fn handle_new_items(
        sink: &SubscriptionSink,
        subscription_type: SubscriptionType,
        new_items: Vec<PubSubResult>,
        filter: Option<&PubSubFilter>,
    ) -> Result<(), SendTimeoutError> {
        for item in new_items {
            if let PubSubResult::Log(log) = &item {
                if let Some(filter) = &filter {
                    if !filter.matches(log) {
                        continue;
                    }
                }
            }

            sink.send_timeout(
                SubscriptionMessage::from_json(&item)
                    .expect("PubSubResult always serializable to json;qed"),
                SUBSCRIPTION_SINK_SEND_TIMEOUT,
            )
            .await?;

            olaos_logs::info!("notify {:?}", subscription_type);
        }

        olaos_logs::info!("notify {:?} new items finished", subscription_type);
        Ok(())
    }

    #[olaos_logs::instrument(skip(self, pending_sink))]
    pub async fn sub(
        &self,
        pending_sink: PendingSubscriptionSink,
        sub_type: String,
        params: Option<PubSubFilter>,
    ) {
        let sub_type = match sub_type.as_str() {
            "newHeads" => {
                let Ok(sink) = pending_sink.accept().await else {
                    return;
                };
                let blocks_rx = self.blocks.subscribe();
                tokio::spawn(Self::run_subscriber(
                    sink,
                    SubscriptionType::Blocks,
                    blocks_rx,
                    None,
                ));

                Some(SubscriptionType::Blocks)
            }
            "newPendingTransactions" => {
                let Ok(sink) = pending_sink.accept().await else {
                    return;
                };
                let transactions_rx = self.transactions.subscribe();
                tokio::spawn(Self::run_subscriber(
                    sink,
                    SubscriptionType::Txs,
                    transactions_rx,
                    None,
                ));
                Some(SubscriptionType::Txs)
            }
            "logs" => {
                let filter = params.unwrap_or_default();
                let topic_count = filter.topics.as_ref().map_or(0, Vec::len);

                if topic_count > EVENT_TOPIC_NUMBER_LIMIT {
                    Self::reject(pending_sink).await;
                    None
                } else {
                    let Ok(sink) = pending_sink.accept().await else {
                        return;
                    };
                    let logs_rx = self.logs.subscribe();
                    tokio::spawn(Self::run_subscriber(
                        sink,
                        SubscriptionType::Logs,
                        logs_rx,
                        Some(filter),
                    ));
                    Some(SubscriptionType::Logs)
                }
            }
            "syncing" => {
                let Ok(sink) = pending_sink.accept().await else {
                    return;
                };

                tokio::spawn(async move {
                    sink.send_timeout(
                        SubscriptionMessage::from_json(&PubSubResult::Syncing(false)).unwrap(),
                        SUBSCRIPTION_SINK_SEND_TIMEOUT,
                    )
                    .await
                });
                None
            }
            "l1_batch_proofs" => {
                let Ok(sink) = pending_sink.accept().await else {
                    return;
                };
                let block_proofs_rx = self.l1_batch_proofs.subscribe();
                tokio::spawn(Self::run_subscriber(
                    sink,
                    SubscriptionType::L1BatchProofs,
                    block_proofs_rx,
                    None,
                ));

                Some(SubscriptionType::L1BatchProofs)
            }
            _ => {
                Self::reject(pending_sink).await;
                None
            }
        };

        if let Some(sub_type) = sub_type {
            if let Some(sender) = &self.events_sender {
                sender.send(PubSubEvent::Subscribed(sub_type)).ok();
            }
        }
    }

    /// Spawns notifier tasks. This should be called once per instance.
    pub fn spawn_notifiers(
        &self,
        connection_pool: ConnectionPool,
        polling_interval: Duration,
        stop_receiver: watch::Receiver<bool>,
    ) -> Vec<JoinHandle<anyhow::Result<()>>> {
        let mut notifier_tasks = Vec::with_capacity(3);

        // let notifier = PubSubNotifier {
        //     sender: self.blocks.clone(),
        //     connection_pool: connection_pool.clone(),
        //     polling_interval,
        //     events_sender: self.events_sender.clone(),
        // };
        // let notifier_task = tokio::spawn(notifier.notify_blocks(stop_receiver.clone()));
        // notifier_tasks.push(notifier_task);

        // let notifier = PubSubNotifier {
        //     sender: self.transactions.clone(),
        //     connection_pool: connection_pool.clone(),
        //     polling_interval,
        //     events_sender: self.events_sender.clone(),
        // };
        // let notifier_task = tokio::spawn(notifier.notify_txs(stop_receiver.clone()));
        // notifier_tasks.push(notifier_task);

        // let notifier = PubSubNotifier {
        //     sender: self.logs.clone(),
        //     connection_pool,
        //     polling_interval,
        //     events_sender: self.events_sender.clone(),
        // };
        // let notifier_task = tokio::spawn(notifier.notify_logs(stop_receiver));

        let notifier = PubSubNotifier {
            sender: self.l1_batch_proofs.clone(),
            connection_pool,
            polling_interval: Duration::from_secs(10),
            events_sender: self.events_sender.clone(),
        };
        let notifier_task = tokio::spawn(notifier.notify_l1_batch_proofs(stop_receiver));

        notifier_tasks.push(notifier_task);
        notifier_tasks
    }
}

#[async_trait::async_trait]
impl EthPubSubServer for EthSubscribe {
    async fn subscribe(
        &self,
        pending: PendingSubscriptionSink,
        sub_type: String,
        filter: Option<PubSubFilter>,
    ) -> SubscriptionResult {
        self.sub(pending, sub_type, filter).await;
        Ok(())
    }
}
