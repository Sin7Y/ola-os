use std::{
    collections::HashMap,
    fmt,
    time::{Duration, Instant},
};

use itertools::Itertools;

use ola_types::{
    block::MiniblockReexecuteData,
    fee::TransactionExecutionMetrics,
    get_nonce_key,
    l2::{L2Tx, L2TxCommonData},
    request::PaymasterParams,
    tx::{execute::Execute, tx_execution_info::TxExecutionStatus, TransactionExecutionResult},
    Address, ExecuteTransactionCommon, MiniblockNumber, Nonce, PriorityOpId, Transaction, H256,
    U256,
};
use ola_utils::{h256_to_u32, u256_to_big_decimal};

use crate::{
    models::storage_transaction::StorageTransaction, time_utils::pg_interval_from_duration,
    StorageProcessor,
};
use sqlx::{
    error,
    types::{chrono::NaiveDateTime, BigDecimal},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum L2TxSubmissionResult {
    Added,
    Replaced,
    AlreadyExecuted,
    Duplicate,
    Proxied,
}

impl fmt::Display for L2TxSubmissionResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug)]
pub struct TransactionsDal<'c, 'a> {
    pub(crate) storage: &'c mut StorageProcessor<'a>,
}

impl TransactionsDal<'_, '_> {
    pub async fn insert_transaction_l2(
        &mut self,
        tx: L2Tx,
        exec_info: TransactionExecutionMetrics,
    ) -> L2TxSubmissionResult {
        {
            let tx_hash = tx.hash();
            let initiator_address = tx.initiator_account();
            let contract_address = tx.execute.contract_address.as_bytes();
            let json_data = serde_json::to_value(&tx.execute)
                .unwrap_or_else(|_| panic!("cannot serialize tx {:?} to json", tx.hash()));
            let signature = tx.common_data.signature;
            let nonce = tx.common_data.nonce.0 as i64;
            let input_data = tx.common_data.input.expect("Data is mandatory").data;
            let paymaster = tx.common_data.paymaster_params.paymaster.0.as_ref();
            let paymaster_input = tx.common_data.paymaster_params.paymaster_input;
            let secs = (tx.received_timestamp_ms / 1000) as i64;
            let nanosecs = ((tx.received_timestamp_ms % 1000) * 1_000_000) as u32;
            let received_at = NaiveDateTime::from_timestamp_opt(secs, nanosecs).unwrap();
            // Besides just adding or updating(on conflict) the record, we want to extract some info
            // from the query below, to indicate what actually happened:
            // 1) transaction is added
            // 2) transaction is replaced
            // 3) WHERE clause conditions for DO UPDATE block were not met, so the transaction can't be replaced
            // the subquery in RETURNING clause looks into pre-UPDATE state of the table. So if the subquery will return NULL
            // transaction is fresh and was added to db(the second condition of RETURNING clause checks it).
            // Otherwise, if the subquery won't return NULL it means that there is already tx with such nonce and initiator_address in DB
            // and we can replace it WHERE clause conditions are met.
            // It is worth mentioning that if WHERE clause conditions are not met, None will be returned.
            let query_result = sqlx::query!(
                r#"
                INSERT INTO transactions
                (
                    hash,
                    is_priority,
                    initiator_address,
                    nonce,
                    signature,
                    gas_limit,
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                    gas_per_pubdata_limit,
                    input,
                    data,
                    tx_format,
                    contract_address,
                    value,
                    paymaster,
                    paymaster_input,
                    execution_info,
                    received_at,
                    created_at,
                    updated_at
                )
                VALUES
                    (
                        $1, FALSE, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                        jsonb_build_object('gas_used', $16::bigint, 'storage_writes', $17::int, 'contracts_used', $18::int),
                        $19, now(), now()
                    )
                ON CONFLICT
                    (initiator_address, nonce)
                DO UPDATE
                    SET hash=$1,
                        signature=$4,
                        gas_limit=$5,
                        max_fee_per_gas=$6,
                        max_priority_fee_per_gas=$7,
                        gas_per_pubdata_limit=$8,
                        input=$9,
                        data=$10,
                        tx_format=$11,
                        contract_address=$12,
                        value=$13,
                        paymaster=$14,
                        paymaster_input=$15,
                        execution_info=jsonb_build_object('gas_used', $16::bigint, 'storage_writes', $17::int, 'contracts_used', $18::int),
                        in_mempool=FALSE,
                        received_at=$19,
                        created_at=now(),
                        updated_at=now(),
                        error = NULL
                    WHERE transactions.is_priority = FALSE AND transactions.miniblock_number IS NULL
                    RETURNING (SELECT hash FROM transactions WHERE transactions.initiator_address = $2 AND transactions.nonce = $3) IS NOT NULL as "is_replaced!"
                "#,
                tx_hash.as_bytes(),
                initiator_address.as_bytes(),
                nonce,
                &signature,
                // FIXME: remove gas in database
                BigDecimal::new(0.into(), 1),
                BigDecimal::new(0.into(), 1),
                BigDecimal::new(0.into(), 1),
                BigDecimal::new(0.into(), 1),
                input_data,
                &json_data,
                0,
                contract_address,
                BigDecimal::new(0.into(), 1),
                &paymaster,
                &paymaster_input,
                0,
                (exec_info.initial_storage_writes + exec_info.repeated_storage_writes) as i32,
                exec_info.contracts_used as i32,
                received_at
            )
                .fetch_optional(self.storage.conn())
                .await
                .map(|option_record| option_record.map(|record| record.is_replaced));

            let l2_tx_insertion_result = match query_result {
                Ok(option_query_result) => match option_query_result {
                    Some(true) => L2TxSubmissionResult::Replaced,
                    Some(false) => L2TxSubmissionResult::Added,
                    None => L2TxSubmissionResult::AlreadyExecuted,
                },
                Err(err) => {
                    // So, we consider a tx hash to be a primary key of the transaction
                    // Based on the idea that we can't have two transactions with the same hash
                    // We assume that if there already exists some transaction with some tx hash
                    // another tx with the same tx hash is supposed to have the same data
                    // In this case we identify it as Duplicate
                    // Note, this error can happen because of the race condition (tx can be taken by several
                    // api servers, that simultaneously start execute it and try to inserted to DB)
                    if let error::Error::Database(ref error) = err {
                        if let Some(constraint) = error.constraint() {
                            if constraint == "transactions_pkey" {
                                return L2TxSubmissionResult::Duplicate;
                            }
                        }
                    }
                    panic!("{}", err);
                }
            };
            olaos_logs::debug!(
                "{:?} l2 transaction {:?} to DB. init_acc {:?} nonce {:?} returned option {:?}",
                l2_tx_insertion_result,
                tx_hash,
                initiator_address,
                nonce,
                l2_tx_insertion_result
            );

            l2_tx_insertion_result
        }
    }

    pub async fn next_priority_id(&mut self) -> PriorityOpId {
        {
            sqlx::query!(
                r#"SELECT MAX(priority_op_id) as "op_id" from transactions where is_priority = true AND miniblock_number IS NOT NULL"#
            )
                .fetch_optional(self.storage.conn())
                .await
                .unwrap()
                .and_then(|row| row.op_id)
                .map(|value| PriorityOpId((value + 1) as u64))
                .unwrap_or_default()
        }
    }

    pub async fn mark_txs_as_executed_in_miniblock(
        &mut self,
        miniblock_number: MiniblockNumber,
        transactions: &[TransactionExecutionResult],
    ) {
        {
            let mut transaction = self.storage.start_transaction().await;

            // TODO: l1 transaction is not supported yet. 23/10/23
            // let mut l1_hashes = Vec::with_capacity(transactions.len());
            // let mut l1_indices_in_block = Vec::with_capacity(transactions.len());
            // let mut l1_errors = Vec::with_capacity(transactions.len());
            // let mut l1_execution_infos = Vec::with_capacity(transactions.len());
            // let mut l1_refunded_gas = Vec::with_capacity(transactions.len());
            // let mut l1_effective_gas_prices = Vec::with_capacity(transactions.len());

            let mut upgrade_hashes = Vec::new();
            let mut upgrade_indices_in_block = Vec::new();
            let mut upgrade_errors = Vec::new();
            let mut upgrade_execution_infos = Vec::new();
            let mut upgrade_refunded_gas = Vec::new();
            let mut upgrade_effective_gas_prices = Vec::new();

            let mut l2_hashes = Vec::with_capacity(transactions.len());
            let mut l2_values = Vec::with_capacity(transactions.len());
            let mut l2_contract_addresses = Vec::with_capacity(transactions.len());
            let mut l2_paymaster = Vec::with_capacity(transactions.len());
            let mut l2_paymaster_input = Vec::with_capacity(transactions.len());
            let mut l2_indices_in_block = Vec::with_capacity(transactions.len());
            let mut l2_initiators = Vec::with_capacity(transactions.len());
            let mut l2_nonces = Vec::with_capacity(transactions.len());
            let mut l2_signatures = Vec::with_capacity(transactions.len());
            let mut l2_tx_formats = Vec::with_capacity(transactions.len());
            let mut l2_errors = Vec::with_capacity(transactions.len());
            let mut l2_effective_gas_prices = Vec::with_capacity(transactions.len());
            let mut l2_execution_infos = Vec::with_capacity(transactions.len());
            let mut l2_inputs = Vec::with_capacity(transactions.len());
            let mut l2_datas = Vec::with_capacity(transactions.len());
            let mut l2_gas_limits = Vec::with_capacity(transactions.len());
            let mut l2_max_fees_per_gas = Vec::with_capacity(transactions.len());
            let mut l2_max_priority_fees_per_gas = Vec::with_capacity(transactions.len());
            let mut l2_gas_per_pubdata_limit = Vec::with_capacity(transactions.len());
            let mut l2_refunded_gas = Vec::with_capacity(transactions.len());

            let mut call_traces_tx_hashes = Vec::with_capacity(transactions.len());
            let mut bytea_call_traces = Vec::with_capacity(transactions.len());
            transactions
                .iter()
                .enumerate()
                .for_each(|(index_in_block, tx_res)| {
                    // TODO: replace with real execution result
                    let TransactionExecutionResult {
                        hash,
                        execution_info,
                        transaction,
                        execution_status,
                        ..
                    } = tx_res;

                    // Bootloader currently doesn't return detailed errors.
                    let error = match execution_status {
                        TxExecutionStatus::Success => None,
                        // The string error used here is copied from the previous version.
                        // It is applied to every failed transaction -
                        // currently detailed errors are not supported.
                        TxExecutionStatus::Failure => Some("Bootloader-based tx failed".to_owned()),
                    };

                    if let Some(call_trace) = tx_res.call_trace() {
                        let started_at = Instant::now();
                        bytea_call_traces.push(bincode::serialize(&call_trace).unwrap());
                        call_traces_tx_hashes.push(hash.0.to_vec());
                    }

                    match &transaction.common_data {
                        ExecuteTransactionCommon::L2(common_data) => {
                            let data = serde_json::to_value(&transaction.execute).unwrap();
                            l2_contract_addresses
                                .push(transaction.execute.contract_address.as_bytes().to_vec());
                            l2_paymaster_input
                                .push(common_data.paymaster_params.paymaster_input.clone());
                            l2_paymaster
                                .push(common_data.paymaster_params.paymaster.as_bytes().to_vec());
                            l2_hashes.push(hash.0.to_vec());
                            l2_indices_in_block.push(index_in_block as i32);
                            l2_initiators.push(transaction.initiator_account().0.to_vec());
                            l2_nonces.push(common_data.nonce.0 as i32);
                            l2_signatures.push(common_data.signature.clone());
                            l2_errors.push(error.unwrap_or_default());
                            l2_execution_infos.push(serde_json::to_value(execution_info).unwrap());
                            // Normally input data is mandatory
                            l2_inputs.push(common_data.input_data().unwrap_or_default());
                            l2_datas.push(data);
                        }
                        ExecuteTransactionCommon::ProtocolUpgrade(common_data) => {
                            upgrade_hashes.push(hash.0.to_vec());
                            upgrade_indices_in_block.push(index_in_block as i32);
                            upgrade_errors.push(error.unwrap_or_default());
                            upgrade_execution_infos
                                .push(serde_json::to_value(execution_info).unwrap());
                            upgrade_refunded_gas.push(0 as i64);
                            upgrade_effective_gas_prices.push(u256_to_big_decimal(U256::from(0)));
                        }
                    }
                });

            if !l2_hashes.is_empty() {
                // Update l2 txs

                // Due to the current tx replacement model, it's possible that tx has been replaced,
                // but the original was executed in memory,
                // so we have to update all fields for tx from fields stored in memory.
                // Note, that transactions are updated in order of their hashes to avoid deadlocks with other UPDATE queries.
                sqlx::query!(
                    r#"
                        UPDATE transactions
                            SET 
                                hash = data_table.hash,
                                signature = data_table.signature,
                                gas_limit = data_table.gas_limit,
                                max_fee_per_gas = data_table.max_fee_per_gas,
                                max_priority_fee_per_gas = data_table.max_priority_fee_per_gas,
                                gas_per_pubdata_limit = data_table.gas_per_pubdata_limit,
                                input = data_table.input,
                                data = data_table.data,
                                tx_format = data_table.tx_format,
                                miniblock_number = $21,
                                index_in_block = data_table.index_in_block,
                                error = NULLIF(data_table.error, ''),
                                effective_gas_price = data_table.effective_gas_price,
                                execution_info = data_table.new_execution_info,
                                refunded_gas = data_table.refunded_gas,
                                value = data_table.value,
                                contract_address = data_table.contract_address,
                                paymaster = data_table.paymaster,
                                paymaster_input = data_table.paymaster_input,
                                in_mempool = FALSE,
                                updated_at = now()
                        FROM
                            (
                                SELECT data_table_temp.* FROM (
                                    SELECT
                                        UNNEST($1::bytea[]) AS initiator_address,
                                        UNNEST($2::int[]) AS nonce,
                                        UNNEST($3::bytea[]) AS hash,
                                        UNNEST($4::bytea[]) AS signature,
                                        UNNEST($5::numeric[]) AS gas_limit,
                                        UNNEST($6::numeric[]) AS max_fee_per_gas,
                                        UNNEST($7::numeric[]) AS max_priority_fee_per_gas,
                                        UNNEST($8::numeric[]) AS gas_per_pubdata_limit,
                                        UNNEST($9::int[]) AS tx_format,
                                        UNNEST($10::integer[]) AS index_in_block,
                                        UNNEST($11::varchar[]) AS error,
                                        UNNEST($12::numeric[]) AS effective_gas_price,
                                        UNNEST($13::jsonb[]) AS new_execution_info,
                                        UNNEST($14::bytea[]) AS input,
                                        UNNEST($15::jsonb[]) AS data,
                                        UNNEST($16::bigint[]) as refunded_gas,
                                        UNNEST($17::numeric[]) as value,
                                        UNNEST($18::bytea[]) as contract_address,
                                        UNNEST($19::bytea[]) as paymaster,
                                        UNNEST($20::bytea[]) as paymaster_input
                                ) AS data_table_temp
                                JOIN transactions ON transactions.initiator_address = data_table_temp.initiator_address
                                    AND transactions.nonce = data_table_temp.nonce
                                ORDER BY transactions.hash
                            ) AS data_table
                        WHERE transactions.initiator_address=data_table.initiator_address
                        AND transactions.nonce=data_table.nonce
                    "#,
                    &l2_initiators,
                    &l2_nonces,
                    &l2_hashes,
                    &l2_signatures,
                    &l2_gas_limits,
                    &l2_max_fees_per_gas,
                    &l2_max_priority_fees_per_gas,
                    &l2_gas_per_pubdata_limit,
                    &l2_tx_formats,
                    &l2_indices_in_block,
                    &l2_errors,
                    &l2_effective_gas_prices,
                    &l2_execution_infos,
                    &l2_inputs as &[&[u8]],
                    &l2_datas,
                    &l2_refunded_gas,
                    &l2_values,
                    &l2_contract_addresses,
                    &l2_paymaster,
                    &l2_paymaster_input,
                    miniblock_number.0 as i32,
                )
                .execute(transaction.conn())
                .await
                .unwrap();
            }

            // TODO: l1 transaction is not supported yet. 23/10/23
            // // We can't replace l1 transaction, so we simply write the execution result
            // if !l1_hashes.is_empty() {
            //     sqlx::query!(
            //         r#"
            //             UPDATE transactions
            //                 SET
            //                     miniblock_number = $1,
            //                     index_in_block = data_table.index_in_block,
            //                     error = NULLIF(data_table.error, ''),
            //                     in_mempool=FALSE,
            //                     execution_info = execution_info || data_table.new_execution_info,
            //                     refunded_gas = data_table.refunded_gas,
            //                     effective_gas_price = data_table.effective_gas_price,
            //                     updated_at = now()
            //             FROM
            //                 (
            //                     SELECT
            //                         UNNEST($2::bytea[]) AS hash,
            //                         UNNEST($3::integer[]) AS index_in_block,
            //                         UNNEST($4::varchar[]) AS error,
            //                         UNNEST($5::jsonb[]) AS new_execution_info,
            //                         UNNEST($6::bigint[]) as refunded_gas,
            //                         UNNEST($7::numeric[]) as effective_gas_price
            //                 ) AS data_table
            //             WHERE transactions.hash = data_table.hash
            //         "#,
            //         miniblock_number.0 as i32,
            //         &l1_hashes,
            //         &l1_indices_in_block,
            //         &l1_errors,
            //         &l1_execution_infos,
            //         &l1_refunded_gas,
            //         &l1_effective_gas_prices,
            //     )
            //     .execute(transaction.conn())
            //     .await
            //     .unwrap();
            // }

            if !upgrade_hashes.is_empty() {
                sqlx::query!(
                    r#"
                        UPDATE transactions
                            SET
                                miniblock_number = $1,
                                index_in_block = data_table.index_in_block,
                                error = NULLIF(data_table.error, ''),
                                in_mempool=FALSE,
                                execution_info = execution_info || data_table.new_execution_info,
                                refunded_gas = data_table.refunded_gas,
                                effective_gas_price = data_table.effective_gas_price,
                                updated_at = now()
                        FROM
                            (
                                SELECT
                                    UNNEST($2::bytea[]) AS hash,
                                    UNNEST($3::integer[]) AS index_in_block,
                                    UNNEST($4::varchar[]) AS error,
                                    UNNEST($5::jsonb[]) AS new_execution_info,
                                    UNNEST($6::bigint[]) as refunded_gas,
                                    UNNEST($7::numeric[]) as effective_gas_price
                            ) AS data_table
                        WHERE transactions.hash = data_table.hash
                    "#,
                    miniblock_number.0 as i32,
                    &upgrade_hashes,
                    &upgrade_indices_in_block,
                    &upgrade_errors,
                    &upgrade_execution_infos,
                    &upgrade_refunded_gas,
                    &upgrade_effective_gas_prices,
                )
                .execute(transaction.conn())
                .await
                .unwrap();
            }

            if !bytea_call_traces.is_empty() {
                let started_at = Instant::now();
                sqlx::query!(
                    r#"
                        INSERT INTO call_traces (tx_hash, call_trace)
                        SELECT u.tx_hash, u.call_trace
                        FROM UNNEST($1::bytea[], $2::bytea[])
                        AS u(tx_hash, call_trace)
                        "#,
                    &call_traces_tx_hashes,
                    &bytea_call_traces
                )
                .execute(transaction.conn())
                .await
                .unwrap();
            }
            transaction.commit().await;
        }
    }

    pub async fn remove_stuck_txs(&mut self, stuck_tx_timeout: Duration) -> usize {
        {
            let stuck_tx_timeout = pg_interval_from_duration(stuck_tx_timeout);
            sqlx::query!(
                "DELETE FROM transactions \
                 WHERE miniblock_number IS NULL AND received_at < now() - $1::interval \
                 AND is_priority=false AND error IS NULL \
                 RETURNING hash",
                stuck_tx_timeout
            )
            .fetch_all(self.storage.conn())
            .await
            .unwrap()
            .len()
        }
    }

    pub async fn reset_mempool(&mut self) {
        {
            sqlx::query!("UPDATE transactions SET in_mempool = FALSE WHERE in_mempool = TRUE")
                .execute(self.storage.conn())
                .await
                .unwrap();
        }
    }

    pub async fn sync_mempool(
        &mut self,
        stashed_accounts: Vec<Address>,
        purged_accounts: Vec<Address>,
        limit: usize,
    ) -> (Vec<Transaction>, HashMap<Address, Nonce>) {
        {
            let stashed_addresses: Vec<_> =
                stashed_accounts.into_iter().map(|a| a.0.to_vec()).collect();
            sqlx::query!(
                "UPDATE transactions SET in_mempool = FALSE \
                FROM UNNEST ($1::bytea[]) AS s(address) \
                WHERE transactions.in_mempool = TRUE AND transactions.initiator_address = s.address",
                &stashed_addresses,
            )
            .execute(self.storage.conn())
            .await
            .unwrap();

            let purged_addresses: Vec<_> =
                purged_accounts.into_iter().map(|a| a.0.to_vec()).collect();
            sqlx::query!(
                "DELETE FROM transactions \
                WHERE in_mempool = TRUE AND initiator_address = ANY($1)",
                &purged_addresses[..]
            )
            .execute(self.storage.conn())
            .await
            .unwrap();

            // Note, that transactions are updated in order of their hashes to avoid deadlocks with other UPDATE queries.
            let transactions = sqlx::query_as!(
                StorageTransaction,
                "UPDATE transactions
                SET in_mempool = TRUE
                FROM (
                    SELECT hash FROM (
                        SELECT hash
                        FROM transactions
                        WHERE miniblock_number IS NULL AND in_mempool = FALSE AND error IS NULL
                            AND (is_priority = TRUE OR (max_fee_per_gas >= $2 and gas_per_pubdata_limit >= $3))
                            AND tx_format != $4
                        ORDER BY is_priority DESC, priority_op_id, received_at
                        LIMIT $1
                    ) as subquery1
                    ORDER BY hash
                ) as subquery2
                WHERE transactions.hash = subquery2.hash
                RETURNING transactions.*",
                limit as i32,
                BigDecimal::from(0),
                BigDecimal::from(0),
                0 as i32,
            )
            .fetch_all(self.storage.conn())
            .await
            .unwrap();

            let nonce_keys: HashMap<_, _> = transactions
                .iter()
                .map(|tx| {
                    let address = Address::from_slice(&tx.initiator_address);
                    let nonce_key = get_nonce_key(&address).hashed_key();
                    (nonce_key, address)
                })
                .collect();

            let storage_keys: Vec<_> = nonce_keys.keys().map(|key| key.0.to_vec()).collect();
            let nonces: HashMap<_, _> = sqlx::query!(
                r#"SELECT hashed_key, value as "value!" FROM storage WHERE hashed_key = ANY($1)"#,
                &storage_keys,
            )
            .fetch_all(self.storage.conn())
            .await
            .unwrap()
            .into_iter()
            .map(|row| {
                let nonce_key = H256::from_slice(&row.hashed_key);
                let nonce = Nonce(h256_to_u32(H256::from_slice(&row.value)));

                (*nonce_keys.get(&nonce_key).unwrap(), nonce)
            })
            .collect();

            (
                transactions.into_iter().map(|tx| tx.into()).collect(),
                nonces,
            )
        }
    }

    pub async fn get_miniblocks_to_reexecute(&mut self) -> Vec<MiniblockReexecuteData> {
        let transactions_by_miniblock: Vec<(MiniblockNumber, Vec<Transaction>)> = sqlx::query_as!(
            StorageTransaction,
            "SELECT * FROM transactions \
            WHERE miniblock_number IS NOT NULL AND l1_batch_number IS NULL \
            ORDER BY miniblock_number, index_in_block",
        )
        .fetch_all(self.storage.conn())
        .await
        .unwrap()
        .into_iter()
        .group_by(|tx| tx.miniblock_number.unwrap())
        .into_iter()
        .map(|(miniblock_number, txs)| {
            (
                MiniblockNumber(miniblock_number as u32),
                txs.map(Transaction::from).collect::<Vec<_>>(),
            )
        })
        .collect();
        if transactions_by_miniblock.is_empty() {
            return Vec::new();
        }

        let from_miniblock = transactions_by_miniblock.first().unwrap().0;
        let to_miniblock = transactions_by_miniblock.last().unwrap().0;
        let timestamps = sqlx::query!(
            "SELECT timestamp FROM miniblocks WHERE number BETWEEN $1 AND $2 ORDER BY number",
            from_miniblock.0 as i64,
            to_miniblock.0 as i64,
        )
        .fetch_all(self.storage.conn())
        .await
        .unwrap();

        transactions_by_miniblock
            .into_iter()
            .zip(timestamps)
            .map(|((number, txs), row)| MiniblockReexecuteData {
                number,
                timestamp: row.timestamp as u64,
                txs,
            })
            .collect()
    }

    pub(crate) async fn get_tx_by_hash(&mut self, hash: H256) -> Option<Transaction> {
        sqlx::query_as!(
            StorageTransaction,
            r#"
                SELECT * FROM transactions
                WHERE hash = $1
            "#,
            hash.as_bytes()
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(|tx| tx.into())
    }
}

impl From<StorageTransaction> for L2TxCommonData {
    fn from(tx: StorageTransaction) -> Self {
        let nonce = Nonce(tx.nonce.expect("no nonce in L2 tx in DB") as u32);

        let StorageTransaction {
            paymaster,
            paymaster_input,
            initiator_address,
            signature,
            hash,
            input,
            ..
        } = tx;

        let paymaster_params = PaymasterParams {
            paymaster: Address::from_slice(&paymaster),
            paymaster_input,
        };

        L2TxCommonData::new(
            nonce,
            Address::from_slice(&initiator_address),
            signature.unwrap_or_else(|| {
                panic!("Signature is mandatory for transactions. Tx {:#?}", hash)
            }),
            input.expect("input data is mandatory for l2 transactions"),
            H256::from_slice(&hash),
            paymaster_params,
        )
    }
}

impl From<StorageTransaction> for Transaction {
    fn from(tx: StorageTransaction) -> Self {
        let hash = H256::from_slice(&tx.hash);
        let execute = serde_json::from_value::<Execute>(tx.data.clone())
            .unwrap_or_else(|_| panic!("invalid json in database for tx {:?}", hash));
        let received_timestamp_ms = tx.received_at.timestamp_millis() as u64;
        Transaction {
            common_data: ExecuteTransactionCommon::L2(tx.into()),
            execute,
            received_timestamp_ms,
        }
    }
}
