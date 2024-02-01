use std::{collections::HashMap, path::Path, sync::Arc, time::Instant};

use ola_dal::StorageProcessor;
use ola_types::{L1BatchNumber, StorageKey, StorageValue, H256};
use olaos_storage::db::{NamedColumnFamily, RocksDB};

use crate::{in_memory::InMemoryStorage, ReadStorage};

fn serialize_block_number(block_number: u32) -> [u8; 4] {
    block_number.to_le_bytes()
}

fn deserialize_block_number(bytes: &[u8]) -> u32 {
    let bytes: [u8; 4] = bytes.try_into().expect("incorrect block number format");
    u32::from_le_bytes(bytes)
}

#[derive(Debug, Clone, Copy)]
pub enum SequencerColumnFamily {
    State,
    Contracts,
    FactoryDeps,
}

impl NamedColumnFamily for SequencerColumnFamily {
    const DB_NAME: &'static str = "sequencer";
    const ALL: &'static [Self] = &[Self::State, Self::Contracts, Self::FactoryDeps];

    fn name(&self) -> &'static str {
        match self {
            Self::State => "state",
            Self::Contracts => "contracts",
            Self::FactoryDeps => "factory_deps",
        }
    }
}

/// [`ReadStorage`] implementation backed by RocksDB.
#[derive(Debug)]
pub struct RocksdbStorage {
    db: Arc<RocksDB<SequencerColumnFamily>>,
    pending_patch: InMemoryStorage,
}

impl RocksdbStorage {
    const BLOCK_NUMBER_KEY: &'static [u8] = b"block_number";

    /// Creates a new storage with the provided RocksDB `path`.
    pub fn new(path: &Path) -> Self {
        let db = RocksDB::new(path, true);
        Self {
            db: Arc::new(db),
            pending_patch: InMemoryStorage::default(),
        }
    }

    pub async fn update_from_postgres(&mut self, conn: &mut StorageProcessor<'_>) {
        let _stage_started_at: Instant = Instant::now();
        let latest_l1_batch_number = conn.blocks_dal().get_sealed_l1_batch_number().await;
        olaos_logs::info!(
            "loading storage for l1 batch number {}",
            latest_l1_batch_number.0
        );

        let mut current_l1_batch_number = self.l1_batch_number().0;
        assert!(
            current_l1_batch_number <= latest_l1_batch_number.0 + 1,
            "L1 batch number in sequencer cache ({current_l1_batch_number}) is greater than \
             the last sealed L1 batch number in Postgres ({latest_l1_batch_number})"
        );

        while current_l1_batch_number <= latest_l1_batch_number.0 {
            olaos_logs::info!("loading state changes for l1 batch {current_l1_batch_number}");
            let storage_logs = conn
                .storage_logs_dal()
                .get_touched_slots_for_l1_batch(L1BatchNumber(current_l1_batch_number))
                .await;
            self.process_transaction_logs(&storage_logs);

            olaos_logs::info!("loading factory deps for l1 batch {current_l1_batch_number}");
            let factory_deps = conn
                .blocks_dal()
                .get_l1_batch_factory_deps(L1BatchNumber(current_l1_batch_number))
                .await;
            for (hash, bytecode) in factory_deps {
                self.store_factory_dep(hash, bytecode);
            }

            current_l1_batch_number += 1;
            self.save(L1BatchNumber(current_l1_batch_number)).await;
        }
    }

    async fn save(&mut self, l1_batch_number: L1BatchNumber) {
        let pending_patch = std::mem::take(&mut self.pending_patch);

        let db = Arc::clone(&self.db);
        let save_task = tokio::task::spawn_blocking(move || {
            let mut batch = db.new_write_batch();
            let cf = SequencerColumnFamily::State;
            batch.put_cf(
                cf,
                Self::BLOCK_NUMBER_KEY,
                &serialize_block_number(l1_batch_number.0),
            );
            for (key, value) in pending_patch.state {
                batch.put_cf(cf, &Self::serialize_state_key(&key), value.as_ref());
            }

            let cf = SequencerColumnFamily::FactoryDeps;
            for (hash, value) in pending_patch.factory_deps {
                batch.put_cf(cf, &hash.to_fixed_bytes(), value.as_ref());
            }
            db.write(batch)
                .expect("failed to save state data into rocksdb");
        });
        save_task.await.unwrap();
    }

    pub fn l1_batch_number(&self) -> L1BatchNumber {
        let cf = SequencerColumnFamily::State;
        let block_number = self
            .db
            .get_cf(cf, Self::BLOCK_NUMBER_KEY)
            .expect("failed to fetch block number");
        let block_number = block_number.map_or(0, |bytes| deserialize_block_number(&bytes));
        L1BatchNumber(block_number)
    }

    pub fn read_value_inner(&self, key: &StorageKey) -> Option<StorageValue> {
        let cf = SequencerColumnFamily::State;
        self.db
            .get_cf(cf, &Self::serialize_state_key(key))
            .expect("failed to read rocksdb state value")
            .map(|value| H256::from_slice(&value))
    }

    fn process_transaction_logs(&mut self, updates: &HashMap<StorageKey, H256>) {
        for (&key, &value) in updates {
            if !value.is_zero() || self.read_value_inner(&key).is_some() {
                self.pending_patch.state.insert(key, value);
            }
        }
    }

    pub fn store_factory_dep(&mut self, hash: H256, bytecode: Vec<u8>) {
        self.pending_patch.factory_deps.insert(hash, bytecode);
    }

    fn serialize_state_key(key: &StorageKey) -> [u8; 32] {
        key.hashed_key().to_fixed_bytes()
    }

    pub fn estimated_map_size(&self) -> u64 {
        self.db
            .estimated_number_of_entries(SequencerColumnFamily::State)
    }
}

impl ReadStorage for &RocksdbStorage {
    fn read_value(&mut self, key: &StorageKey) -> StorageValue {
        self.read_value_inner(key).unwrap_or_else(H256::zero)
    }

    fn is_write_initial(&mut self, key: &StorageKey) -> bool {
        self.read_value_inner(key).is_none()
    }

    fn load_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>> {
        if let Some(value) = self.pending_patch.factory_deps.get(&hash) {
            return Some(value.clone());
        }
        let cf = SequencerColumnFamily::FactoryDeps;
        self.db
            .get_cf(cf, hash.as_bytes())
            .expect("failed to read RocksDB state value")
    }
}
