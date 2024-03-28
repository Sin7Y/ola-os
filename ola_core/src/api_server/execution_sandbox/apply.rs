use ola_config::{database::load_db_config, sequencer::load_network_config};
use ola_dal::{connection::ConnectionPool, SqlxError, StorageProcessor};
use ola_executor::{tx_exe_manager::OlaTapeInitInfo, tx_pre_executor::TxPreExecutor};
use ola_types::{
    api::{BlockId, BlockNumber},
    ExecuteTransactionCommon, L1BatchNumber, MiniblockNumber, Transaction,
};
use ola_utils::{bytes_to_u64s, h256_to_u64_array, time::seconds_since_epoch};
use olavm_core::util::converts::u8_arr_to_address;

use super::{BlockArgs, TxSharedArgs, VmPermit};

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_vm_in_sandbox(
    vm_permit: VmPermit,
    shared_args: TxSharedArgs,
    connection_pool: &ConnectionPool,
    tx: Transaction,
    block_args: BlockArgs,
) -> anyhow::Result<()> {
    let rt_handle = vm_permit.rt_handle();
    let mut connection = rt_handle.block_on(connection_pool.access_storage_tagged("api"));

    let (state_block_number, vm_block_number) = rt_handle
        .block_on(block_args.resolve_block_numbers(&mut connection))
        .expect("Failed resolving block numbers");

    if block_args.resolves_to_latest_sealed_miniblock() {
        shared_args
            .caches
            .schedule_values_update(state_block_number);
    }
    let block_timestamp = block_args.block_timestamp_seconds();

    let db_config = load_db_config().expect("failed to load database config");
    let network = load_network_config().expect("failed to load network config");

    let hash = tx.hash();
    let calldata = tx.execute.calldata;
    let tx = match &tx.common_data {
        ExecuteTransactionCommon::L2(tx) => {
            let to_u8_32 = |v: &Vec<u8>| {
                let mut array = [0; 32];
                array.copy_from_slice(&v[..32]);
                array
            };

            let r = tx.signature[0..32].to_vec();
            let s = tx.signature[32..64].to_vec();
            let signature_r = bytes_to_u64s(to_u8_32(&r).to_vec());
            let signature_s = bytes_to_u64s(to_u8_32(&s).to_vec());
            OlaTapeInitInfo {
                version: tx.transaction_type as u64,
                origin_address: h256_to_u64_array(&tx.initiator_address),
                calldata: bytes_to_u64s(calldata),
                nonce: Some(tx.nonce.0 as u64),
                signature_r: Some([
                    signature_r.get(0).unwrap().clone(),
                    signature_r.get(1).unwrap().clone(),
                    signature_r.get(2).unwrap().clone(),
                    signature_r.get(3).unwrap().clone(),
                ]),
                signature_s: Some([
                    signature_s.get(0).unwrap().clone(),
                    signature_s.get(1).unwrap().clone(),
                    signature_s.get(2).unwrap().clone(),
                    signature_s.get(3).unwrap().clone(),
                ]),
                tx_hash: Some(h256_to_u64_array(&hash)),
            }
        }
        ExecuteTransactionCommon::ProtocolUpgrade(_) => panic!("ProtocolUpgrade not supported"),
    };

    let mut pre_executor = TxPreExecutor::new(
        db_config.sequencer_db_path,
        network.ola_network_id as u64,
        vm_block_number.0 as u64,
        block_timestamp,
        u8_arr_to_address(&shared_args.operator_account.address().to_fixed_bytes()),
    )?;

    drop(vm_permit);
    pre_executor.invoke(tx)
}

impl BlockArgs {
    fn is_pending_miniblock(&self) -> bool {
        matches!(self.block_id, BlockId::Number(BlockNumber::Pending))
    }

    fn resolves_to_latest_sealed_miniblock(&self) -> bool {
        matches!(
            self.block_id,
            BlockId::Number(BlockNumber::Pending | BlockNumber::Latest | BlockNumber::Committed)
        )
    }

    async fn resolve_block_numbers(
        &self,
        connection: &mut StorageProcessor<'_>,
    ) -> Result<(MiniblockNumber, L1BatchNumber), SqlxError> {
        Ok(if self.is_pending_miniblock() {
            let sealed_l1_batch_number =
                connection.blocks_dal().get_sealed_l1_batch_number().await?;
            let sealed_miniblock_number = connection
                .blocks_dal()
                .get_sealed_miniblock_number()
                .await?;
            (sealed_miniblock_number, sealed_l1_batch_number + 1)
        } else {
            let l1_batch_number = connection
                .storage_web3_dal()
                .resolve_l1_batch_number_of_miniblock(self.resolved_block_number)
                .await?
                .expected_l1_batch();
            (self.resolved_block_number, l1_batch_number)
        })
    }

    fn block_timestamp_seconds(&self) -> u64 {
        if self.is_pending_miniblock() {
            seconds_since_epoch()
        } else {
            self.block_timestamp_s.unwrap_or_else(|| {
                panic!(
                    "Block timestamp is `None`, `block_id`: {:?}, `resolved_block_number`: {}",
                    self.block_id, self.resolved_block_number.0
                );
            })
        }
    }
}
