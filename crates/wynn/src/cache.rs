//! Provides [`Cache`] that contains wynncraft api loop states.
//!
//! The cache can be accessed via client data.
//!
//! Outside the api loop, if you want information on the wynncraft api without making a http
//! request, try read it from [`Cache`] instead.
//! ```
//! use std::sync::Arc;
//!
//! use serenity::client::Context;
//! use serenity::model::channel::Message;
//! use serenity::framework::standard::CommandResult;
//! use wynn::cache::Cache;
//!
//! async fn display_guild_level(ctx: &Context, msg: &Message) -> CommandResult {
//!     // Get cache from bot data
//!     let cache: Arc<Cache> = {
//!         let data = ctx.data.read().await;
//!         let cache = data.get::<Cache>().expect("Failed to get cache");
//!         Arc::clone(cache)
//!     };
//!     // Get guild level from cache
//!     let level = {
//!         let guild_data = cache.guild.read().await;
//!         match guild_data.as_ref() {
//!             Some(guild_data) => guild_data.level,
//!             None => {
//!                 msg.reply(ctx, "No guild data cached").await?;
//!                 return Ok(())
//!             }
//!         }
//!     };
//!
//!     msg.reply(ctx, format!("The guild level is {}", level)).await?;
//!     Ok(())
//! }
//! ```
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use serenity::prelude::TypeMapKey;
use tokio::sync::RwLock;

use crate::model::{Guild, GuildMember};
use util::{read_json, write_json};

/// Container for the event loops' persistent data
#[derive(Debug, Default)]
pub struct Cache {
    /// Guild statistic.
    ///
    /// Note that the field [`members`] is empty, for its value, see [`Cache.members`].
    ///
    /// [`members`]: crate::model::Guild::members
    pub guild: RwLock<Option<Guild>>,
    /// The guild members, a map between id and its corresponding guild member data.
    pub members: RwLock<Option<MemberMap>>,
    /// Online players, a map between the server name and its list of online player names.
    ///
    /// Note that this map only contains the names of players who has a linked wynn profile in the
    /// member database.
    pub online: RwLock<OnlineMap>,
}

impl Cache {
    /// Read cache from file
    pub async fn new() -> Result<Self> {
        Ok(Self {
            guild: RwLock::new(read_json!("cache/guild.json")),
            members: RwLock::new(read_json!("cache/members.json")),
            online: RwLock::new(OnlineMap(HashMap::new())),
        })
    }

    /// Write cache to file
    pub async fn write(&self) {
        write_json!("cache/guild.json", &*self.guild.read().await, "guild");
        write_json!("cache/members.json", &*self.members.read().await, "members");
    }
}

/// Bot data key for [`Cache`].
/// ```
/// use std::sync::Arc;
///
/// use serenity::client::Context;
/// use wynn::cache::Cache;
///
/// async fn get_cache_from_ctx(ctx: &Context) -> Arc<Cache> {
///     let data = ctx.data.read().await;
///     let cache = data.get::<Cache>().expect("Failed to get cache");
///     Arc::clone(cache)
/// }
/// ```
impl TypeMapKey for Cache {
    type Value = Arc<Cache>;
}

/// Map between mcid and guild member object
pub type MemberMap = HashMap<String, GuildMember>;

/// Map of worlds and player igns in that world.
///
/// This map assumes each ign is unique across all map values, aka no multiple worlds contains the
/// same ign.
#[derive(Debug, Default)]
pub struct OnlineMap(pub HashMap<String, HashSet<String>>);

impl OnlineMap {
    /// Get the world which the given ign is contained in
    pub fn world(&self, ign: &str) -> Option<&str> {
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
