use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ola_types::{
    tx::tx_execution_info::ExecutionMetrics, Address, Nonce, PriorityOpId, Transaction,
};
use olaos_mempool::mempool_store::{MempoolInfo, MempoolStore};

#[derive(Debug, Clone)]
pub struct MempoolGuard(Arc<Mutex<MempoolStore>>);

impl MempoolGuard {
    pub fn new(next_priority_id: PriorityOpId, capacity: u64) -> Self {
        let store = MempoolStore::new(next_priority_id, capacity);
        Self(Arc::new(Mutex::new(store)))
    }

    pub fn insert(&mut self, transactions: Vec<Transaction>, nonces: HashMap<Address, Nonce>) {
        self.0
            .lock()
            .expect("failed to acquire mempool lock")
            .insert(transactions, nonces);
    }

    pub fn has_next(&self) -> bool {
        self.0
            .lock()
            .expect("failed to acquire mempool lock")
            .has_next()
    }

    pub fn next_transaction(&mut self) -> Option<Transaction> {
        self.0
            .lock()
            .expect("failed to acquire mempool lock")
            .next_transaction()
    }

    pub fn get_mempool_info(&mut self) -> MempoolInfo {
        self.0
            .lock()
            .expect("failed to acquire mempool lock")
            .get_mempool_info()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExecutionMetricsForCriteria {
    pub execution_metrics: ExecutionMetrics,
}
