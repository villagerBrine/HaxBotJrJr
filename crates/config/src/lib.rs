//! Provides tools for managing bot configuration data
//!
//! The [`Config`] struct contains all configuration data and can be access from bot data.
//! ```
//! use std::sync::Arc;
//!
//! use serenity::client::Context;
//! use serenity::model::channel::{Message, Channel};
//! use serenity::framework::standard::CommandResult;
//! use tokio::sync::RwLock;
//! use config::Config;
//!
//! async fn check_if_channel_tracked(ctx: &Context, msg: &Message) -> CommandResult {
//!     // Get config from bot data
//!     let config: Arc<RwLock<Config>> = {
//!         let data = ctx.data.read().await;
//!         let cache = data.get::<Config>().expect("Failed to get config");
//!         Arc::clone(cache)
//!     };
//!     // Get the channel the command was send in
//!     let channel = match msg.channel(&ctx).await? {
//!         Channel::Guild(c) => c,
//!         // Guild only command, so this part is unreachable
//!         _ => unreachable!()
//!     };
//!     // Checks if that channel is track through `Config`
//!     let is_tracked = {
//!         let config = config.read().await;
//!         config.is_channel_tracked(&ctx.cache, &channel)
//!     };
//!
//!     msg.reply(ctx, if is_tracked {
//!         "This channel is tracked"
//!     } else {
//!         "This channel is not tracked"
//!     }).await?;
//!     Ok(())
//! }
//! ```
#[warn(missing_docs, missing_debug_implementations)]
pub mod tag;
pub mod utils;

use std::sync::Arc;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serenity::client::Cache;
use serenity::http::CacheHttp;
use serenity::model::channel::{Channel, GuildChannel};
use serenity::model::guild::Member;
use serenity::prelude::TypeMapKey;
use tag::{ChannelTag, TagMap, TextChannelTag, UserTag};
use tokio::sync::RwLock;
use tracing::info;

use event::{DiscordEvent, DiscordSignal};
use util::{read_json, some, write_json};

/// Configuration data
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub channel_tags: TagMap<u64, ChannelTag>,
    pub category_tags: TagMap<u64, ChannelTag>,
    pub text_channel_tags: TagMap<u64, TextChannelTag>,
    pub user_tags: TagMap<u64, UserTag>,
    pub user_role_tags: TagMap<u64, UserTag>,
}

impl Config {
    /// Load config from file
    pub fn new(file: &str) -> Option<Self> {
        read_json!(file, Self::default())
    }

    /// Write config to file
    pub fn write(&self, path: &str) {
        write_json!(path, &self, "config");
    }

    /// Helper function for checking if a channel has a tag, both directly and indirectly
    ///
    /// [`ChannelTag`] is inheritable, meaning if a category has a tag, then it also applies to the
    /// channels under it; The same rule applies to channel and its threads.
    ///
    /// Note that [`TextChannelTag`] isn't inheritable, so using [`TagMap::tag`] on
    /// [`text_channel_tags`] to check if a channel has a `TextChannelTag` is sufficient.
    ///
    /// [`ChannelTag`]: crate::tag::ChannelTag
    /// [`TextChannelTag`]: crate::tag::TextChannelTag
    /// [`TagMap::tag`]: crate::tag::TagMap::tag
    /// [`text_channel_tags`]: crate::Config::text_channel_tags
    fn check_channel_tag(&self, cache: &Cache, channel: &GuildChannel, tag: &ChannelTag) -> bool {
        if self.channel_tags.tagged(&channel.id.0, tag) {
            return false;
        }

        // Checks if its parent channels has the tag
        let (category_id, parent_id) = util::discord::get_channel_parents(cache, channel);

        if let Some(id) = category_id {
            if self.channel_tags.tagged(&id.0, tag) {
                return false;
            }
        }
        if let Some(id) = parent_id {
            if self.channel_tags.tagged(&id.0, tag) {
                return false;
            }
        }
        true
    }

    /// Helper function for checking if a discord member has a tag, both directly and indirectly
    ///
    /// [`UserTag`] is inheritable, meaning if a role has a tag, then it also applies to any users
    /// with that role.
    ///
    /// [`UserTag`]: crate::Tag::UserTag
    fn check_memebr_tag(&self, member: &Member, tag: &UserTag) -> bool {
        if self.user_tags.tagged(&member.user.id.0, tag) {
            return false;
        }
        for role in &member.roles {
            if self.user_role_tags.tagged(&role.0, tag) {
                return false;
            }
        }
        true
    }

    /// Checks if a channel can be used for stats tracking
    pub fn is_channel_tracked(&self, cache: &Cache, channel: &GuildChannel) -> bool {
        self.check_channel_tag(cache, channel, &ChannelTag::NoTrack)
    }

    /// Checks if a discord member's nick can be automatically updated by the bot
    pub fn should_update_nick(&self, member: &Member) -> bool {
        self.check_memebr_tag(member, &UserTag::NoNickUpdate)
    }

    /// Checks if a discord member's roles can be automatically updated by the bot
    pub fn should_update_role(&self, member: &Member) -> bool {
        self.check_memebr_tag(member, &UserTag::NoRoleUpdate)
    }

    /// Send a message to all the channels with given [`TextChannelTag`]
    ///
    /// [`TextChannelTag`]: crate::tag::TextChannelTag
    pub async fn send(&self, cache_http: &impl CacheHttp, tag: &TextChannelTag, content: &str) -> Result<()> {
        let cache = some!(cache_http.cache(), bail!("No cache"));
        let http = cache_http.http();
        for channel_id in self.text_channel_tags.tagged_objects(tag) {
            if let Some(Channel::Guild(channel)) = cache.channel(*channel_id) {
                channel.say(http, content).await?;
            }
        }
        Ok(())
    }
}

/// Discord data key for [`Config`]
/// ```
/// use std::sync::Arc;
///
/// use serenity::client::Context;
/// use tokio::sync::RwLock;
/// use config::Config;
///
/// async fn get_config_from_ctx(ctx: &Context) -> Arc<RwLock<Config>> {
///     let data = ctx.data.read().await;
///     let config = data.get::<Config>().expect("Failed to get config");
///     Arc::clone(config)
/// }
/// ```
impl TypeMapKey for Config {
    type Value = Arc<RwLock<Config>>;
}

/// Start the loop that keeps the [`Config`] up to date
pub async fn start_loop(config: Arc<RwLock<Config>>, signal: DiscordSignal) {
    tokio::spawn(async move {
        info!("starting config manage loop (discord event)");
        let mut receiver = signal.connect();
        loop {
            let event = receiver.recv().await.unwrap();
            let (_ctx, event) = event.as_ref();
            process_discord_event(&config, event).await;
        }
    });
}

/// Update config based on discord event
async fn process_discord_event(config: &RwLock<Config>, event: &DiscordEvent) {
    match event {
        DiscordEvent::ChannelDelete { channel } => {
            info!("Discord channel deleted, updating config");
            let mut config = config.write().await;
            config.channel_tags.remove_all(&channel.id.0);
            config.category_tags.remove_all(&channel.id.0);
            config.text_channel_tags.remove_all(&channel.id.0);
        }
        DiscordEvent::RoleDelete { id, .. } => {
            info!("Discord role deleted, updating config");
            let mut config = config.write().await;
            config.user_role_tags.remove_all(&id.0);
        }
        _ => {}
    }
}
