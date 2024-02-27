use std::time::Duration;

use ola_utils::env_tools::parse_env;
use sqlx::{
    pool::PoolConnection,
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Postgres,
};

use crate::{
    get_master_database_url, get_prover_database_url, get_replica_database_url, StorageProcessor,
};

pub mod holder;

const OLAOS_DATABASE_POOL_SIZE: u32 = 50;

#[derive(Debug, Clone, Copy)]
pub enum DbVariant {
    Master,
    Replica,
    Prover,
}

#[derive(Debug)]
pub struct ConnectionPoolBuilder {
    db: DbVariant,
    max_size: Option<u32>,
    statement_timeout: Option<Duration>,
}

impl ConnectionPoolBuilder {
    pub fn set_max_size(&mut self, max_size: Option<u32>) -> &mut Self {
        self.max_size = max_size;
        self
    }

    pub fn set_statement_timeout(&mut self, timeout: Option<Duration>) -> &mut Self {
        self.statement_timeout = timeout;
        self
    }

    pub async fn build(&self) -> ConnectionPool {
        let db_url = match self.db {
            DbVariant::Master => get_master_database_url(),
            DbVariant::Replica => get_replica_database_url(),
            DbVariant::Prover => get_prover_database_url(),
        };
        self.build_inner(&db_url).await
    }

    pub async fn build_inner(&self, db_url: &str) -> ConnectionPool {
        let max_connections = self.max_size.unwrap_or_else(|| OLAOS_DATABASE_POOL_SIZE);
        let options = PgPoolOptions::new().max_connections(max_connections);
        let mut connect_options: PgConnectOptions = db_url.parse().unwrap_or_else(|e| {
            panic!("Failed parsing {:?} database URL: {}", self.db, e);
        });
        if let Some(timeout) = self.statement_timeout {
            let timeout_string = format!("{}s", timeout.as_secs());
            connect_options = connect_options.options([("statement_timeout", timeout_string)]);
        }
        let pool = options
            .connect_with(connect_options)
            .await
            .unwrap_or_else(|err| {
                panic!("Failed connecting to {:?}, error: {}", self.db, err);
            });
        ConnectionPool::Real(pool)
    }
}

#[derive(Debug, Clone)]
pub enum ConnectionPool {
    Real(PgPool),
    Test(PgPool),
}

impl ConnectionPool {
    pub fn builder(db: DbVariant) -> ConnectionPoolBuilder {
        ConnectionPoolBuilder {
            db,
            max_size: None,
            statement_timeout: None,
        }
    }

    pub fn singleton(db: DbVariant) -> ConnectionPoolBuilder {
        ConnectionPoolBuilder {
            db,
            max_size: Some(1),
            statement_timeout: None,
        }
    }

    pub async fn access_storage(&self) -> StorageProcessor {
        self.access_storage_inner(None).await
    }

    pub async fn access_storage_tagged(&self, requester: &'static str) -> StorageProcessor<'_> {
        self.access_storage_inner(Some(requester)).await
    }

    async fn access_storage_inner(&self, _requester: Option<&'static str>) -> StorageProcessor<'_> {
        match self {
            ConnectionPool::Real(real_pool) => {
                let conn = Self::acquire_connection_retried(real_pool).await;
                StorageProcessor::from_pool(conn)
            }
            ConnectionPool::Test(_test_pool) => {
                panic!("test pool not supported!")
            }
        }
    }

    async fn acquire_connection_retried(pool: &PgPool) -> PoolConnection<Postgres> {
        const DB_CONNECTION_RETRIES: u32 = 3;
        const BACKOFF_INTERVAL: Duration = Duration::from_secs(1);

        let mut retry_count = 0;
        while retry_count < DB_CONNECTION_RETRIES {
            let connection = pool.acquire().await;
            match connection {
                Ok(connection) => return connection,
                Err(_) => {
                    retry_count += 1;
                }
            };

            tokio::time::sleep(BACKOFF_INTERVAL).await;
        }
        pool.acquire()
            .await
            .unwrap_or_else(|err| panic!("Failed getting a DB connection: {}", err))
    }

    pub fn max_size(&self) -> u32 {
        // TODO:
        4
    }
}
