use std::env;

use blocks_web3_dal::BlocksWeb3Dal;
use connection::holder::ConnectionHolder;
pub use sqlx::Error as SqlxError;
use sqlx::{pool::PoolConnection, Connection, PgConnection, Postgres};
use storage_web3_dal::StorageWeb3Dal;
use token_dal::TokensDal;
use transactions_dal::TransactionsDal;

pub mod blocks_web3_dal;
pub mod connection;
pub mod healthcheck;
pub mod models;
pub mod storage_web3_dal;
pub mod token_dal;
pub mod transactions_dal;

pub fn get_master_database_url() -> String {
    // FIXME:
    // env::var("DATABASE_URL").expect("DATABASE_URL must be set")
    if env::var("OLAOS_IN_DOCKER")
        .expect("OLAOS_IN_DOCKER must be set")
        .parse()
        .unwrap_or(false)
    {
        "postgres://postgres:password@host.docker.internal:5432/olaos".into()
    } else {
        "postgres://postgres:password@localhost:5432/olaos".into()
    }
}

pub fn get_replica_database_url() -> String {
    // FIXME:
    env::var("OLAOS_DATABASE_REPLICA_URL").unwrap_or_else(|_| get_master_database_url())
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

    pub fn from_pool(conn: PoolConnection<Postgres>) -> Self {
        Self {
            conn: ConnectionHolder::Pooled(conn),
            in_transaction: false,
        }
    }

    pub fn tokens_dal(&mut self) -> TokensDal<'_, 'a> {
        TokensDal { storage: self }
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

    fn conn(&mut self) -> &mut PgConnection {
        match &mut self.conn {
            ConnectionHolder::Pooled(conn) => conn,
            ConnectionHolder::Direct(conn) => conn,
            ConnectionHolder::Transaction(conn) => conn,
        }
    }
}
