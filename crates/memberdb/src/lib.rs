//! Module for managing and interacting with the member database
//! You should only access the database via this module.
pub mod api;
pub mod error;
pub mod events;
pub mod loops;
pub mod model;
pub mod query;
pub mod utils;
pub mod voice_tracker;

use std::sync::Arc;

use anyhow::{Context, Result};
use serenity::prelude::TypeMapKey;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;

pub use crate::api::fetch::*;
pub use crate::api::table;
pub use crate::api::update::*;
use crate::events::{DBEvent, DBSignal};

pub type Conn = PoolConnection<Sqlite>;

#[derive(Debug)]
/// A database connection
pub struct DB {
    pub pool: Pool<Sqlite>,
    pub signal: DBSignal,
}

impl DB {
    /// Connect to the database
    pub async fn new(file: &str, max_conn: u32) -> Self {
        Self {
            pool: connect_db(file, max_conn).await,
            signal: DBSignal::new(64),
        }
    }

    /// Begin a transaction
    pub async fn begin(&self) -> Result<Transaction> {
        let tx = self.pool.begin().await.context("Failed to begin db transaction")?;
        return Ok(Transaction { tx, signal: self.signal.clone() });
    }

    /// Get an event receiver
    pub fn connect(&self) -> Receiver<Arc<DBEvent>> {
        self.signal.connect()
    }

    /// Broadcast an event
    pub fn signal(&self, event: DBEvent) {
        self.signal.signal(event);
    }
}

#[derive(Debug)]
/// A database transaction
pub struct Transaction {
    pub tx: sqlx::Transaction<'static, Sqlite>,
    pub signal: DBSignal,
}

impl Transaction {
    /// Commit the transaction
    pub async fn commit(self) -> Result<()> {
        self.tx.commit().await.context("Failed to commit db transaction")
    }

    /// Broadcast an event
    pub fn signal(&self, event: DBEvent) {
        self.signal.signal(event);
    }
}

/// Connect to the database
pub async fn connect_db(file: &str, max_conn: u32) -> Pool<Sqlite> {
    let db = SqlitePoolOptions::new()
        .max_connections(max_conn)
        .connect_with(SqliteConnectOptions::new().filename(file).create_if_missing(true))
        .await
        .expect("Couldn't connect to database");
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("Couldn't run database migrations");
    db
}

/// Bot data key for `DB`
pub struct DBContainer;

impl TypeMapKey for DBContainer {
    type Value = Arc<RwLock<DB>>;
}

impl DBContainer {
    /// Create a `DB` container
    pub async fn new(file: &str, max_conn: u32) -> Arc<RwLock<DB>> {
        let db = DB::new(file, max_conn).await;
        Arc::new(RwLock::new(db))
    }
}
