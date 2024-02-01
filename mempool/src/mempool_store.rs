use std::collections::{hash_map::Entry, BTreeSet, HashMap, HashSet};

use ola_types::{l2::L2Tx, Address, ExecuteTransactionCommon, Nonce, PriorityOpId, Transaction};

use crate::types::{AccountTransactions, MempoolScore};

#[derive(Debug, Default)]
pub struct MempoolStore {
    /// Pending L2 transactions grouped by initiator address
    l2_transactions_per_account: HashMap<Address, AccountTransactions>,
    /// Global priority queue for L2 transactions. Used for scoring
    l2_priority_queue: BTreeSet<MempoolScore>,
    /// Next priority operation
    next_priority_id: PriorityOpId,
    stashed_accounts: Vec<Address>,
    /// number of l2 transactions in the mempool
    size: u64,
    capacity: u64,
}

impl MempoolStore {
    pub fn new(next_priority_id: PriorityOpId, capacity: u64) -> Self {
        Self {
            l2_transactions_per_account: HashMap::new(),
            l2_priority_queue: BTreeSet::new(),
            next_priority_id,
            stashed_accounts: vec![],
            size: 0,
            capacity,
        }
    }

    pub fn has_next(&self) -> bool {
        // TODO: add filter latter
        self.l2_priority_queue.iter().rfind(|_| true).is_some()
    }

    #[olaos_logs::instrument(skip(self))]
    pub fn next_transaction(&mut self) -> Option<Transaction> {
        let mut removed = 0;
        // We want to fetch the next transaction that would match the fee requirements.
        // TODO: add filter
        let tx_pointer = self.l2_priority_queue.iter().rfind(|_el| true)?.clone();

        // Stash all observed transactions that don't meet criteria
        for stashed_pointer in self
            .l2_priority_queue
            .split_off(&tx_pointer)
            .into_iter()
            .skip(1)
        {
            removed += self
                .l2_transactions_per_account
                .remove(&stashed_pointer.account)
                .expect("mempool: dangling pointer in priority queue")
                .len();

            self.stashed_accounts.push(stashed_pointer.account);
        }
        // insert pointer to the next transaction if it exists
        let (transaction, score) = self
            .l2_transactions_per_account
            .get_mut(&tx_pointer.account)
            .expect("mempool: dangling pointer in priority queue")
            .next();

        if let Some(score) = score {
            self.l2_priority_queue.insert(score);
        }
        self.size = self
            .size
            .checked_sub((removed + 1) as u64)
            .expect("mempool size can't be negative");
        olaos_logs::info!(
            "get next transaction {:?} mempool size {:?}",
            transaction.hash(),
            self.size
        );
        Some(transaction.into())
    }

    pub fn insert(
        &mut self,
        transactions: Vec<Transaction>,
        initial_nonces: HashMap<Address, Nonce>,
    ) {
        for transaction in transactions {
            let Transaction {
                common_data,
                execute,
                received_timestamp_ms,
            } = transaction;
            match common_data {
                ExecuteTransactionCommon::L2(data) => {
                    olaos_logs::info!(
                        "inserting L2 transaction, address {:?} nonce {}",
                        data.initiator_address,
                        data.nonce
                    );
                    self.insert_l2_transaction(
                        L2Tx {
                            execute,
                            common_data: data,
                            received_timestamp_ms,
                        },
                        &initial_nonces,
                    );
                }
                ExecuteTransactionCommon::ProtocolUpgrade(_) => {
                    olaos_logs::error!(
                        "Protocol upgrade tx is not supposed to be inserted into mempool"
                    );
                    panic!("Protocol upgrade tx is not supposed to be inserted into mempool");
                }
            }
        }
    }

    #[olaos_logs::instrument(skip(self))]
    fn insert_l2_transaction(
        &mut self,
        transaction: L2Tx,
        initial_nonces: &HashMap<Address, Nonce>,
    ) {
        let account = transaction.initiator_account();

        let metadata = match self.l2_transactions_per_account.entry(account) {
            Entry::Occupied(mut txs) => txs.get_mut().insert(transaction),
            Entry::Vacant(entry) => {
                let account_nonce = initial_nonces.get(&account).cloned().unwrap_or(Nonce(0));
                olaos_logs::info!(
                    "insert new account {:?}, nonce {:?}",
                    account,
                    account_nonce
                );
                entry
                    .insert(AccountTransactions::new(account_nonce))
                    .insert(transaction)
            }
        };
        if let Some(score) = metadata.previous_score {
            olaos_logs::info!("remove previous score {:?}", score);
            self.l2_priority_queue.remove(&score);
        }
        if let Some(score) = metadata.new_score {
            olaos_logs::info!("insert new score {:?}", score);
            self.l2_priority_queue.insert(score);
        }
        if metadata.is_new {
            olaos_logs::info!("add transaction size");
            self.size += 1;
        }
    }

    /// When a sequencer starts the block over after a rejected transaction,
    /// we have to rollback the nonces/ids in the mempool and
    /// reinsert the transactions from the block back into mempool.
    #[olaos_logs::instrument(skip(self))]
    pub fn rollback(&mut self, tx: &Transaction) {
        // rolling back the nonces and priority ids
        match &tx.common_data {
            ExecuteTransactionCommon::L2(_) => {
                if let Some(score) = self
                    .l2_transactions_per_account
                    .get_mut(&tx.initiator_account())
                    .expect("account is not available in mempool")
                    .reset(tx)
                {
                    self.l2_priority_queue.remove(&score);
                }
            }
            ExecuteTransactionCommon::ProtocolUpgrade(_) => {
                panic!("Protocol upgrade tx is not supposed to be in mempool");
            }
        }
    }

    pub fn get_mempool_info(&mut self) -> MempoolInfo {
        MempoolInfo {
            stashed_accounts: std::mem::take(&mut self.stashed_accounts),
            purged_accounts: self.gc(),
        }
    }

    fn gc(&mut self) -> Vec<Address> {
        if self.size >= self.capacity {
            let index: HashSet<_> = self
                .l2_priority_queue
                .iter()
                .map(|pointer| pointer.account)
                .collect();
            let transactions = std::mem::take(&mut self.l2_transactions_per_account);
            let (kept, drained) = transactions
                .into_iter()
                .partition(|(address, _)| index.contains(address));
            self.l2_transactions_per_account = kept;
            self.size = self
                .l2_transactions_per_account
                .iter()
                .fold(0, |agg, (_, tnxs)| agg + tnxs.len() as u64);
            olaos_logs::info!(
                "drained addresses in gc {:?}",
                drained.keys().collect::<Vec<_>>()
            );
            return drained.into_keys().collect();
        }
        vec![]
    }
}

#[derive(Debug)]
pub struct MempoolInfo {
    pub stashed_accounts: Vec<Address>,
    pub purged_accounts: Vec<Address>,
}
