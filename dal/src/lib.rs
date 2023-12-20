use std::env;

use blocks_dal::BlocksDal;
use blocks_web3_dal::BlocksWeb3Dal;
use connection::holder::ConnectionHolder;
use events_dal::EventsDal;
use protocol_version_dal::ProtocolVersionsDal;
pub use sqlx::Error as SqlxError;
use sqlx::{pool::PoolConnection, Connection, PgConnection, Postgres, Transaction};
use storage_dal::StorageDal;
use storage_logs_dal::StorageLogsDal;
use storage_logs_dedup_dal::StorageLogsDedupDal;
use storage_web3_dal::StorageWeb3Dal;
use token_dal::TokensDal;
use transaction_web3_dal::TransactionsWeb3Dal;
use transactions_dal::TransactionsDal;

#[macro_use]
mod macro_utils;
pub mod blocks_dal;
pub mod blocks_web3_dal;
pub mod connection;
pub mod events_dal;
pub mod healthcheck;
pub mod models;
pub mod protocol_version_dal;
pub mod storage_dal;
pub mod storage_logs_dal;
pub mod storage_logs_dedup_dal;
pub mod storage_web3_dal;
pub mod time_utils;
pub mod token_dal;
pub mod transactions_dal;
pub mod transaction_web3_dal;

pub fn get_master_database_url() -> String {
    // FIXME:
    // env::var("DATABASE_URL").expect("DATABASE_URL must be set")
    if env::var("OLAOS_IN_DOCKER")
        .unwrap_or_else(|_| "false".to_string())
        .parse()
        .unwrap_or(false)
    {
        "postgres://admin:admin123@host.docker.internal:5432/olaos".into()
    } else {
        "postgres://admin:admin123@localhost:5432/olaos".into()
    }
}

pub fn get_replica_database_url() -> String {
    // FIXME:
    // env::var("OLAOS_DATABASE_REPLICA_URL").unwrap_or_else(|_| get_master_database_url())
    if env::var("OLAOS_IN_DOCKER")
        .unwrap_or_else(|_| "false".to_string())
        .parse()
        .unwrap_or(false)
    {
        "postgres://admin:admin123@host.docker.internal:5433/olaos".into()
    } else {
        "postgres://admin:admin123@localhost:5433/olaos".into()
    }
}

pub fn get_prover_database_url() -> String {
    env::var("OLAOS_DATABASE_PROVER_URL").unwrap_or_else(|_| get_master_database_url())
}

#[derive(Debug)]
pub struct StorageProcessor<'a> {
    conn: ConnectionHolder<'a>,
    in_transaction: bool,
}

impl<'a> StorageProcessor<'a> {
    pub async fn establish_connection(connection_to_master: bool) -> StorageProcessor<'static> {
        let db_url = if connection_to_master {
            get_master_database_url()
        } else {
            get_replica_database_url()
        };
        let connection = PgConnection::connect(&db_url).await.unwrap();
        StorageProcessor {
            conn: ConnectionHolder::Direct(connection),
            in_transaction: false,
        }
    }

    pub async fn start_transaction<'c: 'b, 'b>(&'c mut self) -> StorageProcessor<'b> {
        let transaction = self.conn().begin().await.unwrap();

        let mut processor = StorageProcessor::from_transaction(transaction);
        processor.in_transaction = true;

        processor
    }

    pub fn from_transaction(conn: Transaction<'a, Postgres>) -> Self {
        Self {
            conn: ConnectionHolder::Transaction(conn),
            in_transaction: true,
        }
    }

    pub fn from_pool(conn: PoolConnection<Postgres>) -> Self {
        Self {
            conn: ConnectionHolder::Pooled(conn),
            in_transaction: false,
        }
    }

    pub fn tokens_dal(&mut self) -> TokensDal<'_, 'a> {
        TokensDal { storage: self }
    }

    pub fn blocks_dal(&mut self) -> BlocksDal<'_, 'a> {
        BlocksDal { storage: self }
    }

    pub fn blocks_web3_dal(&mut self) -> BlocksWeb3Dal<'_, 'a> {
        BlocksWeb3Dal { storage: self }
    }

    pub fn storage_web3_dal(&mut self) -> StorageWeb3Dal<'_, 'a> {
        StorageWeb3Dal { storage: self }
    }

    pub fn transactions_dal(&mut self) -> TransactionsDal<'_, 'a> {
        TransactionsDal { storage: self }
    }

    pub fn transactions_web3_dal(&mut self) -> TransactionsWeb3Dal<'_, 'a> {
        TransactionsWeb3Dal { storage: self }
    }

    pub fn storage_dal(&mut self) -> StorageDal<'_, 'a> {
        StorageDal { storage: self }
    }

    pub fn storage_logs_dal(&mut self) -> StorageLogsDal<'_, 'a> {
        StorageLogsDal { storage: self }
    }

    pub fn storage_logs_dedup_dal(&mut self) -> StorageLogsDedupDal<'_, 'a> {
        StorageLogsDedupDal { storage: self }
    }

    pub fn events_dal(&mut self) -> EventsDal<'_, 'a> {
        EventsDal { storage: self }
    }

    pub fn protocol_versions_dal(&mut self) -> ProtocolVersionsDal<'_, 'a> {
        ProtocolVersionsDal { storage: self }
    }

    fn conn(&mut self) -> &mut PgConnection {
        match &mut self.conn {
            ConnectionHolder::Pooled(conn) => conn,
            ConnectionHolder::Direct(conn) => conn,
            ConnectionHolder::Transaction(conn) => conn,
        }
    }

    pub async fn commit(self) {
        if let ConnectionHolder::Transaction(transaction) = self.conn {
            transaction.commit().await.unwrap();
        } else {
            panic!("StorageProcessor::commit can only be invoked after calling StorageProcessor::begin_transaction");
        }
    }
}
