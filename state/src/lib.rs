use std::{cell::RefCell, collections::HashMap, fmt, rc::Rc};

use ola_types::{get_known_code_key, StorageKey, StorageValue, H256};

pub mod cache;
pub mod in_memory;
pub mod postgres;
pub mod rocksdb;
pub mod storage_view;

pub trait ReadStorage: fmt::Debug {
    /// Read value of the key.
    fn read_value(&mut self, key: &StorageKey) -> StorageValue;

    /// Checks whether a write to this storage at the specified `key` would be an initial write.
    /// Roughly speaking, this is the case when the storage doesn't contain `key`, although
    /// in case of mutable storages, the caveats apply (a write to a key that is present
    /// in the storage but was not committed is still an initial write).
    fn is_write_initial(&mut self, key: &StorageKey) -> bool;

    /// Load the factory dependency code by its hash.
    fn load_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>>;

    /// Returns whether a bytecode hash is "known" to the system.
    fn is_bytecode_known(&mut self, bytecode_hash: &H256) -> bool {
        let code_key = get_known_code_key(bytecode_hash);
        self.read_value(&code_key) != H256::zero()
    }
}

/// Functionality to write to the VM storage in a batch.
///
/// So far, this trait is implemented only for [`StorageView`].
pub trait WriteStorage: ReadStorage {
    /// Sets the new value under a given key and returns the previous value.
    fn set_value(&mut self, key: StorageKey, value: StorageValue) -> StorageValue;

    /// Returns a map with the key–value pairs updated by this batch.
    fn modified_storage_keys(&self) -> &HashMap<StorageKey, StorageValue>;

    /// Returns the number of read / write ops for which the value was read from the underlying
    /// storage.
    fn missed_storage_invocations(&self) -> usize;
}

/// Smart pointer to a dynamically typed [`WriteStorage`].
pub type StoragePtr<'a> = Rc<RefCell<&'a mut dyn WriteStorage>>;
