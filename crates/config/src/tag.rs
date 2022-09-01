//! Provides [`Tag`] and [`TagMap`] for tag-based configuration
//!
//! Tags can be attached to an object to "configure" it.
//! You can thinks of them like discord roles, but you can attach them to more than just discord
//! users.
//!
//! Objects and their attached tags are tracked using [`TagMap`].
//! ```
//! use config::tag::{Tag, TagMap};
//!
//! #[derive(Debug, Hash, Eq, PartialEq, Clone)]
//! enum UserStaffTag {
//!     Admin,
//!     Moderator,
//!     Helper,
//! }
//!
//! impl Tag for UserStaffTag {
//!     fn describe(&self) -> &str {
//!         match self {
//!             Self::Admin => "This user is an admin, manages the server",
//!             Self::Moderator => "This user is a moderator, manages the community",
//!             Self::Helper => "This user is a help, ask them if you have a question"
//!         }
//!     }
//! }
//! #
//! # impl std::str::FromStr for UserStaffTag {
//! #     type Err = std::io::Error;
//! #
//! #     fn from_str(s: &str) -> Result<Self, Self::Err> {
//! #         Ok(match s {
//! #             "Admin" => Self::Admin,
//! #             "Moderator" => Self::Moderator,
//! #             "Helper" => Self::Helper,
//! #             _ => return util::ioerr!("Failed to parse '{}' as UserStaffTag", s),
//! #         })
//! #     }
//! # }
//! #
//! # util::impl_debug_display!(UserStaffTag);
//!
//! // Create a map between user ids and `UserStaffTag`
//! let mut map: TagMap<i64, UserStaffTag> = TagMap::new();
//!
//! // Add the tag `UserStaffTag::Admin` to an user.
//! map.add(&123, UserStaffTag::Admin);
//! // Check if a user has `UserStaffTag::Moderator`.
//! let is_mod: bool = map.tagged(&456, &UserStaffTag::Moderator);
//! // Remove `UserStaffTag::Helper` from user, if they have one.
//! map.remove(&123, &UserStaffTag::Helper);
//! ```
//! See also [`crate::utils::Tags`].
use std::collections::hash_map::Keys;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::hash::Hash;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use util::{impl_debug_display, ioerr};

/// All variants of [`ChannelTag`]
pub const CHANNEL_TAGS: [ChannelTag; 1] = [ChannelTag::NoTrack];
/// All variants of [`TextChannelTag`]
pub const TEXT_CHANNEL_TAGS: [TextChannelTag; 5] = [
    TextChannelTag::GuildMemberLog,
    TextChannelTag::GuildLevelLog,
    TextChannelTag::XpLog,
    TextChannelTag::OnlineLog,
    TextChannelTag::Summary,
];
/// All variants of [`UserTag`]
pub const USER_TAGS: [UserTag; 2] = [UserTag::NoNickUpdate, UserTag::NoRoleUpdate];

/// Trait for objects that can behave as tags.
pub trait Tag: Eq + Hash + FromStr + Display + Clone {
    /// Describe the tag, what is it, and what it does
    fn describe(&self) -> &str;
}

/// A map between objects and their attached tags
#[derive(Debug, Serialize, Deserialize)]
pub struct TagMap<K: Eq + Hash + Clone, T: Tag> {
    map: HashMap<K, HashSet<T>>,
}

impl<K: Eq + Hash + Clone, T: Tag> TagMap<K, T> {
    /// Create an empty map
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    /// Get tags of an object
    pub fn get(&self, obj: &K) -> Option<&HashSet<T>> {
        self.map.get(obj)
    }

    /// Get objects in the maps
    pub fn objects(&self) -> Keys<'_, K, HashSet<T>> {
        self.map.keys()
    }

    /// Get objects with given tag
    pub fn tagged_objects<'a>(&'a self, tag: &'a T) -> impl Iterator<Item = &K> + 'a {
        self.map.keys().filter(|k| self.tagged(k, tag))
    }

    /// Check if an object has the given tag
    pub fn tagged(&self, obj: &K, tag: &T) -> bool {
        if let Some(tags) = self.map.get(obj) {
            tags.contains(tag)
        } else {
            false
        }
    }

    /// Add a tag to an object
    pub fn add(&mut self, obj: &K, tag: T) {
        if let Some(tags) = self.map.get_mut(obj) {
            tags.insert(tag);
        } else {
            let mut tags = HashSet::new();
            tags.insert(tag);
            self.map.insert(obj.clone(), tags);
        }
    }

    /// Remove a tag from an object
    pub fn remove(&mut self, obj: &K, tag: &T) {
        if let Some(tags) = self.map.get_mut(obj) {
            tags.remove(tag);
            if tags.is_empty() {
                self.map.remove(obj);
            }
        }
    }

    /// Remove an object from map
    pub fn remove_all(&mut self, obj: &K) {
        self.map.remove(obj);
    }
}

impl<K: Eq + Hash + Clone, T: Tag> Default for TagMap<K, T> {
    /// Alias of [`TagMap::new`]
    fn default() -> Self {
        Self::new()
    }
}

/// Tags to be attached on a discord member
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Clone)]
pub enum UserTag {
    /// Bot won't update the nickname of the tagged.
    NoNickUpdate,
    /// Bot won't update the roles of the tagged.
    NoRoleUpdate,
}

impl Tag for UserTag {
    fn describe(&self) -> &str {
        match self {
            Self::NoNickUpdate => "Nickname won't be automatically updated",
            Self::NoRoleUpdate => "Roles won't be automatically updated",
        }
    }
}

impl FromStr for UserTag {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "NoNickUpdate" => Self::NoNickUpdate,
            "NoRoleUpdate" => Self::NoRoleUpdate,
            _ => return ioerr!("Failed to parse '{}' as UserTag", s),
        })
    }
}

impl_debug_display!(UserTag);

/// Tags to be attached to a discord channel
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Clone)]
pub enum ChannelTag {
    /// Bot won't track statistics in the tagged channel
    NoTrack,
}

impl Tag for ChannelTag {
    fn describe(&self) -> &str {
        match self {
            Self::NoTrack => "Statistics won't be tracked in this channel",
        }
    }
}

impl FromStr for ChannelTag {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "NoTrack" => Self::NoTrack,
            _ => return ioerr!("Failed to parse '{}' as ChannelTag", s),
        })
    }
}

impl_debug_display!(ChannelTag);

/// Tags to be attached to a text channel
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Clone)]
pub enum TextChannelTag {
    /// Bot logs guild member events in tagged channel
    GuildMemberLog,
    /// Bot logs guild level events in tagged channel
    GuildLevelLog,
    /// Bot logs guild member xp contribution events in tagged channel
    XpLog,
    /// Bot logs wynncraft player online status events in tagged channel
    OnlineLog,
    /// Bot logs weekly stat summaries in tagged channel
    Summary,
}

impl Tag for TextChannelTag {
    fn describe(&self) -> &str {
        match self {
            Self::GuildMemberLog => "Logs guild member join, leave, and rank / ign change",
            Self::GuildLevelLog => "Logs guild level up",
            Self::XpLog => "Logs guild member xp contributions",
            Self::OnlineLog => "Logs player join / leave and world change",
            Self::Summary => "Stat leaderboards are posted weekly",
        }
    }
}

impl FromStr for TextChannelTag {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "GuildMemberLog" => Self::GuildMemberLog,
            "GuildLevelLog" => Self::GuildLevelLog,
            "XpLog" => Self::XpLog,
            "OnlineLog" => Self::OnlineLog,
            "Summary" => Self::Summary,
            _ => return ioerr!("Failed to parse '{}' as TextChannelTag", s),
        })
    }
}

impl_debug_display!(TextChannelTag);
