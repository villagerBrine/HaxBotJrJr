//! The Cache struct is used to record persistent data so event loop can work properly
use std::sync::Arc;

use anyhow::Result;
use serenity::prelude::TypeMapKey;
use tokio::sync::RwLock;

use crate::model::{Guild, MemberMap};
use util::{read_json, write_json};

#[derive(Debug)]
/// Container for the event loops' persistent data
pub struct Cache {
    /// Guild statistic from previous loop (the field `members` is empty)
    pub guild: RwLock<Option<Guild>>,
    /// The guild member map from previous loop
    pub members: RwLock<Option<MemberMap>>,
}

impl Cache {
    /// Read cache from file
    pub async fn restore() -> Result<Self> {
        Ok(Self {
            guild: RwLock::new(read_json!("cache/guild.json")),
            members: RwLock::new(read_json!("cache/members.json")),
        })
    }

    /// Write cache to file
    pub async fn store(&self) {
        write_json!("cache/guild.json", &*self.guild.read().await, "guild");
        write_json!("cache/members.json", &*self.members.read().await, "members");
    }
}

/// Bot data key for `Cache` container
pub struct WynnCacheContainer;

impl TypeMapKey for WynnCacheContainer {
    type Value = Arc<Cache>;
}

impl WynnCacheContainer {
    /// Create a new `Cache` container
    pub async fn new() -> Arc<Cache> {
        Arc::new(Cache::restore().await.expect("Failed to restore wynn cache"))
    }
}
