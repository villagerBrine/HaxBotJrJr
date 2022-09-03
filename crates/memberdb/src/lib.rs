//! Module for managing and interacting with the member database
//! You should only access the database via this module.
pub mod api;
pub mod events;
pub mod loops;
pub mod model;
pub mod query;
pub mod utils;
pub mod voice_tracker;

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use serenity::async_trait;
use serenity::prelude::TypeMapKey;
use sqlx::pool::PoolConnection;
use sqlx::query::Map;
use sqlx::sqlite::{SqliteArguments, SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use sqlx::Error;
use sqlx::{Pool, Sqlite};
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;

use wynn::loops::TrackedIgn;

pub use crate::api::fetch::*;
pub use crate::api::table;
pub use crate::api::update::*;
use crate::events::{DBEvent, DBSignal};

pub type Conn = PoolConnection<Sqlite>;

#[derive(Debug)]
/// A database connection
pub struct DB {
    pool: Pool<Sqlite>,
    signal: DBSignal,
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
        Ok(Transaction { tx, signal: self.signal.clone() })
    }

    /// Get an event receiver
    pub fn connect(&self) -> Receiver<Arc<DBEvent>> {
        self.signal.connect()
    }

    /// Broadcast an event
    pub fn signal(&self, event: DBEvent) {
        self.signal.signal(event);
    }

    pub fn exe(&self) -> Executor<'_> {
        Executor::Pool(self)
    }
}

impl TypeMapKey for DB {
    type Value = Arc<RwLock<DB>>;
}

#[derive(Debug)]
/// A database transaction
pub struct Transaction {
    tx: sqlx::Transaction<'static, Sqlite>,
    signal: DBSignal,
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

    pub fn exe(&mut self) -> Executor<'_> {
        Executor::Transaction(self)
    }
}

#[derive(Debug)]
pub enum Executor<'a> {
    Pool(&'a DB),
    Transaction(&'a mut Transaction),
}

type OptionalMap<'q, F> = Map<'q, Sqlite, F, SqliteArguments<'q>>;

macro_rules! query_call {
    ($self:ident, $query:ident, $method:ident) => {
        match $self {
            Executor::Pool(pool) => $query.$method(&pool.pool).await,
            Executor::Transaction(tx) => $query.$method(&mut tx.tx).await,
        }
    };
}

impl<'a> Executor<'a> {
    async fn optional<'q, F, O>(&mut self, query: OptionalMap<'q, F>) -> Result<Option<O>, Error>
    where
        F: FnMut(SqliteRow) -> Result<O, Error> + Send,
        O: Send + Unpin,
    {
        query_call!(self, query, fetch_optional)
    }

    async fn one<'q, F, O>(&mut self, query: OptionalMap<'q, F>) -> Result<O, Error>
    where
        F: FnMut(SqliteRow) -> Result<O, Error> + Send,
        O: Send + Unpin,
    {
        query_call!(self, query, fetch_one)
    }
}

/// Connect to the database
async fn connect_db(file: &str, max_conn: u32) -> Pool<Sqlite> {
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

pub struct TrackedIgnGetter(pub Arc<RwLock<DB>>);

#[async_trait]
impl TrackedIgn for TrackedIgnGetter {
    async fn tracked_ign(&self) -> Result<HashSet<String>> {
        let db = self.0.read().await;
        crate::table::list_igns(&db).await.map(|list| list.into_iter().collect())
    }
}
