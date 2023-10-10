use std::env;

use connection::holder::ConnectionHolder;
pub use sqlx::Error as SqlxError;
use sqlx::{pool::PoolConnection, Connection, PgConnection, Postgres};
use storage_web3_dal::StorageWeb3Dal;

pub mod connection;
pub mod storage_web3_dal;

pub fn get_master_database_url() -> String {
    env::var("DATABASE_URL").expect("DATABASE_URL must be set")
}

pub fn get_replica_database_url() -> String {
    env::var("OLA_DATABASE_REPLICA_URL").unwrap_or_else(|_| get_master_database_url())
}

pub fn get_prover_database_url() -> String {
    env::var("OLA_DATABASE_PROVER_URL").unwrap_or_else(|_| get_master_database_url())
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

    pub fn storage_web3_dal(&mut self) -> StorageWeb3Dal<'_, 'a> {
        StorageWeb3Dal { storage: self }
    }

    fn conn(&mut self) -> &mut PgConnection {
        match &mut self.conn {
            ConnectionHolder::Pooled(conn) => conn,
            ConnectionHolder::Direct(conn) => conn,
            ConnectionHolder::Transaction(conn) => conn,
        }
    }
}
