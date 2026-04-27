mod agents;
mod builds;
mod jobs;
mod metadata;
mod models;
mod organizations;
mod queues;
mod users;

pub use models::*;

use sqlx::PgPool;

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
