//! The Cache struct is used to record persistent data so event loop can work properly
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use serenity::prelude::TypeMapKey;
use tokio::sync::RwLock;

use crate::model::{Guild, GuildMember};
use util::{read_json, write_json};

#[derive(Debug)]
/// Container for the event loops' persistent data
pub struct Cache {
    /// Guild statistic from previous loop (the field `members` is empty)
    pub guild: RwLock<Option<Guild>>,
    /// The guild member map from previous loop
    pub members: RwLock<Option<MemberMap>>,
    /// Online players from previous loop
    pub online: RwLock<OnlineMap>,
}

impl Cache {
    /// Read cache from file
    pub async fn restore() -> Result<Self> {
        Ok(Self {
            guild: RwLock::new(read_json!("cache/guild.json")),
            members: RwLock::new(read_json!("cache/members.json")),
            online: RwLock::new(OnlineMap(HashMap::new())),
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

pub type MemberMap = HashMap<String, GuildMember>;

#[derive(Debug)]
/// Map of worlds and player igns in that world.
/// This map assumes each ign is unique across all map values, aka no multiple worlds contains the
/// same ign.
pub struct OnlineMap(pub HashMap<String, HashSet<String>>);

impl OnlineMap {
    /// Get the world which the given ign is contained in
    pub fn get_world(&self, ign: &str) -> Option<&str> {
        for (world, igns) in self.0.iter() {
            if igns.contains(ign) {
                return Some(world.as_str());
            }
        }
        None
    }

    /// Insert an ign
    pub fn insert(&mut self, world: String, ign: String) -> bool {
        match self.0.get_mut(&world) {
            Some(igns) => igns.insert(ign),
            None => {
                let mut igns = HashSet::new();
                igns.insert(ign);
                self.0.insert(world, igns);
                true
            }
        }
    }

    /// Remove an ign, if it exists then return the world is was in and if there are no igns
    /// contained in that world after the removal.
    pub fn remove(&mut self, ign: &str) -> Option<(&str, bool)> {
        for (world, igns) in self.0.iter_mut() {
            if igns.remove(ign) {
                return Some((world, igns.is_empty()));
            }
        }
        None
    }
}
