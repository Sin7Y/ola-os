use std::collections::HashMap;

use ola_types::{StorageKey, StorageValue, H256};

#[derive(Debug, Default)]
pub struct InMemoryStorage {
    pub(crate) state: HashMap<StorageKey, StorageValue>,
    pub(crate) factory_deps: HashMap<H256, Vec<u8>>,
}
