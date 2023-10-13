use crate::connection::ConnectionPool;
use olaos_health_check::{async_trait, CheckHealth, Health, HealthStatus};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug, Serialize)]
struct ConnectionPoolHealthDetails {
    pool_size: u32,
}

impl ConnectionPoolHealthDetails {
    async fn new(pool: &PgPool) -> Self {
        Self {
            pool_size: pool.size(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionPoolHealthCheck {
    connection_pool: ConnectionPool,
}

impl ConnectionPoolHealthCheck {
    pub fn new(connection_pool: ConnectionPool) -> ConnectionPoolHealthCheck {
        Self { connection_pool }
    }
}

#[async_trait]
impl CheckHealth for ConnectionPoolHealthCheck {
    fn name(&self) -> &'static str {
        "connection_pool"
    }

    async fn check_health(&self) -> Health {
        // This check is rather feeble, plan to make reliable here:
        // https://linear.app/matterlabs/issue/PLA-255/revamp-db-connection-health-check
        self.connection_pool.access_storage().await;

        let mut health = Health::from(HealthStatus::Ready);
        if let ConnectionPool::Real(pool) = &self.connection_pool {
            let details = ConnectionPoolHealthDetails::new(pool).await;
            health = health.with_details(details);
        }
        health
    }
}
