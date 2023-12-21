use std::{cmp::Ordering, collections::HashMap};

use ola_types::{l2::L2Tx, Address, Nonce, Transaction};

#[derive(Debug)]
pub(crate) struct AccountTransactions {
    /// transactions that belong to given account keyed by transaction nonce
    transactions: HashMap<Nonce, L2Tx>,
    /// account nonce in mempool
    /// equals to committed nonce in db + number of transactions sent to sequncer
    nonce: Nonce,
}

impl AccountTransactions {
    pub fn new(nonce: Nonce) -> Self {
        Self {
            transactions: HashMap::new(),
            nonce,
        }
    }

    pub fn insert(&mut self, transaction: L2Tx) -> InsertionMetadata {
        let mut metadata = InsertionMetadata::default();
        let nonce = transaction.common_data.nonce;
        // skip insertion if transaction is old
        if nonce < self.nonce {
            return metadata;
        }
        let new_score = Self::score_for_transaction(&transaction);
        let previous_score = self
            .transactions
            .insert(nonce, transaction)
            .map(|tx| Self::score_for_transaction(&tx));
        metadata.is_new = previous_score.is_none();
        if nonce == self.nonce {
            metadata.new_score = Some(new_score);
            metadata.previous_score = previous_score;
        }
        metadata
    }

    fn score_for_transaction(transaction: &L2Tx) -> MempoolScore {
        MempoolScore {
            account: transaction.initiator_account(),
            received_at_ms: transaction.received_timestamp_ms,
        }
    }

    // Handles transaction rejection. Returns optional score of its successor
    pub fn reset(&mut self, transaction: &Transaction) -> Option<MempoolScore> {
        // current nonce for the group needs to be reset
        let tx_nonce = transaction
            .nonce()
            .expect("nonce is not set for L2 transaction");
        self.nonce = self.nonce.min(tx_nonce);
        self.transactions
            .get(&(tx_nonce + 1))
            .map(Self::score_for_transaction)
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn next(&mut self) -> (L2Tx, Option<MempoolScore>) {
        let transaction = self
            .transactions
            .remove(&self.nonce)
            .expect("missing transaction in mempool");
        self.nonce += 1;
        let score = self
            .transactions
            .get(&self.nonce)
            .map(Self::score_for_transaction);
        (transaction, score)
    }
}

#[derive(Eq, PartialEq, Clone, Debug, Hash)]
pub struct MempoolScore {
    pub account: Address,
    pub received_at_ms: u64,
}

impl Ord for MempoolScore {
    fn cmp(&self, other: &MempoolScore) -> Ordering {
        match self.received_at_ms.cmp(&other.received_at_ms).reverse() {
            Ordering::Equal => {}
            ordering => return ordering,
        }
        self.account.cmp(&other.account)
    }
}

impl PartialOrd for MempoolScore {
    fn partial_cmp(&self, other: &MempoolScore) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Default)]
pub(crate) struct InsertionMetadata {
    pub new_score: Option<MempoolScore>,
    pub previous_score: Option<MempoolScore>,
    pub is_new: bool,
}
