use itertools::Itertools;
use ola_contracts::{BaseSystemContracts, SystemContractCode};

use std::collections::HashMap;

use ola_types::{log::StorageLog, MiniblockNumber, StorageKey, StorageValue, H256};

use crate::StorageProcessor;

#[derive(Debug)]
pub struct StorageDal<'a, 'c> {
    pub(crate) storage: &'a mut StorageProcessor<'c>,
}

impl StorageDal<'_, '_> {
    /// Inserts factory dependencies for a miniblock. Factory deps are specified as a map of
    /// `(bytecode_hash, bytecode)` entries.
    pub async fn insert_factory_deps(
        &mut self,
        block_number: MiniblockNumber,
        factory_deps: &HashMap<H256, Vec<u8>>,
    ) {
        let (bytecode_hashes, bytecodes): (Vec<_>, Vec<_>) = factory_deps
            .iter()
            .map(|dep| (dep.0.as_bytes(), dep.1.as_slice()))
            .unzip();

        // Copy from stdin can't be used here because of 'ON CONFLICT'.
        sqlx::query!(
            "INSERT INTO factory_deps \
            (bytecode_hash, bytecode, miniblock_number, created_at, updated_at) \
            SELECT u.bytecode_hash, u.bytecode, $3, now(), now() \
                FROM UNNEST($1::bytea[], $2::bytea[]) \
                AS u(bytecode_hash, bytecode) \
            ON CONFLICT (bytecode_hash) DO NOTHING",
            &bytecode_hashes as &[&[u8]],
            &bytecodes as &[&[u8]],
            block_number.0 as i64,
        )
        .execute(self.storage.conn())
        .await
        .unwrap();
    }

    pub async fn apply_storage_logs(
        &mut self,
        updates: &[(H256, Vec<StorageLog>)],
    ) -> HashMap<StorageKey, (H256, StorageValue)> {
        let unique_updates: HashMap<_, _> = updates
            .iter()
            .flat_map(|(tx_hash, storage_logs)| {
                storage_logs
                    .iter()
                    .map(move |log| (log.key, (*tx_hash, log.value)))
            })
            .collect();

        let query_parts = unique_updates.iter().map(|(key, (tx_hash, value))| {
            (
                key.hashed_key().0.to_vec(),
                key.address().0.as_slice(),
                key.key().0.as_slice(),
                value.as_bytes(),
                tx_hash.0.as_slice(),
            )
        });
        let (hashed_keys, addresses, keys, values, tx_hashes): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = query_parts.multiunzip();

        // Copy from stdin can't be used here because of 'ON CONFLICT'.
        sqlx::query!(
            "INSERT INTO storage (hashed_key, address, key, value, tx_hash, created_at, updated_at) \
            SELECT u.hashed_key, u.address, u.key, u.value, u.tx_hash, now(), now() \
                FROM UNNEST ($1::bytea[], $2::bytea[], $3::bytea[], $4::bytea[], $5::bytea[]) \
                AS u(hashed_key, address, key, value, tx_hash) \
            ON CONFLICT (hashed_key) \
            DO UPDATE SET tx_hash = excluded.tx_hash, value = excluded.value, updated_at = now()",
            &hashed_keys,
            &addresses as &[&[u8]],
            &keys as &[&[u8]],
            &values as &[&[u8]],
            &tx_hashes as &[&[u8]],
        )
        .execute(self.storage.conn())
        .await
        .unwrap();

        unique_updates
    }

    pub async fn get_base_system_contracts(
        &mut self,
        entrypoint_hash: H256,
        default_aa_hash: H256,
    ) -> BaseSystemContracts {
        let entrypoint_bytecode = self
            .get_factory_dep(entrypoint_hash)
            .await
            .expect("Bootloader code should be present in the database");
        let entrypoint_code = SystemContractCode {
            code: entrypoint_bytecode,
            hash: entrypoint_hash,
        };

        let default_aa_bytecode = self
            .get_factory_dep(default_aa_hash)
            .await
            .expect("Default account code should be present in the database");

        let default_aa_code = SystemContractCode {
            code: default_aa_bytecode,
            hash: default_aa_hash,
        };
        BaseSystemContracts {
            entrypoint: entrypoint_code,
            default_aa: default_aa_code,
        }
    }

    pub async fn get_factory_dep(&mut self, hash: H256) -> Option<Vec<u8>> {
        sqlx::query!(
            "SELECT bytecode FROM factory_deps WHERE bytecode_hash = $1",
            hash.as_bytes(),
        )
        .fetch_optional(self.storage.conn())
        .await
        .unwrap()
        .map(|row| row.bytecode)
    }
}
