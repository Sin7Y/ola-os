use std::fmt;

use ola_types::{fee::TransactionExecutionMetrics, l2::L2Tx};

use crate::StorageProcessor;
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
}
