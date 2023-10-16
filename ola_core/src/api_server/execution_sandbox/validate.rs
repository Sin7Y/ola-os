use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use ola_config::constants::trusted_slots::{TRUSTED_ADDRESS_SLOTS, TRUSTED_TOKEN_SLOTS};
use ola_dal::{connection::ConnectionPool, StorageProcessor};
use ola_types::{l2::L2Tx, Transaction};
use ola_vm::oracles::validation::ValidationTracerParams;

use crate::api_server::execution_sandbox::{apply, execute::TxExecutionArgs};

use super::{BlockArgs, TxSharedArgs, VmPermit};

// FIXME: define real ValidationError
pub type ValidationError = String;

impl TxSharedArgs {
    pub async fn validate_tx_with_pending_state(
        mut self,
        vm_permit: VmPermit,
        connection_pool: ConnectionPool,
        tx: L2Tx,
    ) -> Result<(), ValidationError> {
        let mut connection = connection_pool.access_storage_tagged("api").await;
        let block_args = BlockArgs::pending(&mut connection).await;
        drop(connection);
        self.validate_tx_in_sandbox(connection_pool, vm_permit, tx, block_args)
            .await
    }

    async fn validate_tx_in_sandbox(
        self,
        connection_pool: ConnectionPool,
        vm_permit: VmPermit,
        tx: L2Tx,
        block_args: BlockArgs,
    ) -> Result<(), ValidationError> {
        let stage_started_at = Instant::now();
        let mut connection = connection_pool.access_storage_tagged("api").await;
        let validation_params = get_validation_params(&mut connection, &tx).await;
        drop(connection);

        let execution_args = TxExecutionArgs::for_validation(&tx);
        let execution_mode = execution_args.execution_mode;
        let tx: Transaction = tx.into();
        let (validation_result, _) = tokio::task::spawn_blocking(move || {
            let span = tracing::debug_span!("validate_in_sandbox").entered();
            let result = apply::apply_vm_in_sandbox(
                vm_permit,
                self,
                &execution_args,
                &connection_pool,
                tx,
                block_args,
                HashMap::new(),
                // FIXME: replace real apply Fn
                |tx| Ok(()),
            );
            span.exit();
            result
        })
        .await
        .unwrap();

        validation_result
    }
}

async fn get_validation_params(
    connection: &mut StorageProcessor<'_>,
    tx: &L2Tx,
) -> ValidationTracerParams {
    let start_time = Instant::now();
    let user_address = tx.common_data.initiator_address;
    let paymaster_address = tx.common_data.paymaster_params.paymaster;

    // This method assumes that the number of tokens is relatively low. When it grows
    // we may need to introduce some kind of caching.
    let all_tokens = connection.tokens_dal().get_all_l2_token_addresses().await;
    metrics::gauge!("api.execution.tokens.amount", all_tokens.len() as f64);

    let span = tracing::debug_span!("compute_trusted_slots_for_validation").entered();
    let trusted_slots: HashSet<_> = all_tokens
        .iter()
        .flat_map(|&token| TRUSTED_TOKEN_SLOTS.iter().map(move |&slot| (token, slot)))
        .collect();

    // We currently don't support any specific trusted addresses.
    let trusted_addresses = HashSet::new();

    // The slots the value of which will be added as allowed address on the fly.
    // Required for working with transparent proxies.
    let trusted_address_slots: HashSet<_> = all_tokens
        .into_iter()
        .flat_map(|token| TRUSTED_ADDRESS_SLOTS.iter().map(move |&slot| (token, slot)))
        .collect();

    span.exit();

    ValidationTracerParams {
        user_address,
        paymaster_address,
        trusted_slots,
        trusted_addresses,
        trusted_address_slots,
    }
}
