//! Tags can be attached to an object, and describes how the bot should treat it
use std::collections::hash_map::Keys;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::hash::Hash;
use std::io;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use util::impl_debug_display;

pub const CHANNEL_TAGS: [ChannelTag; 1] = [ChannelTag::NoTrack];
pub const USER_TAGS: [UserTag; 2] = [UserTag::NoNickUpdate, UserTag::NoRoleUpdate];

pub trait Tag: Eq + Hash + FromStr + Display + Clone {
    /// Describe the tag
    fn describe(&self) -> &str;
}

/// Abstraction over a map from object to its attached tags
#[derive(Debug, Serialize, Deserialize)]
pub struct TagMap<K: Eq + Hash + Copy + Clone, T: Tag> {
    map: HashMap<K, HashSet<T>>,
}

impl<K: Eq + Hash + Copy + Clone, T: Tag> TagMap<K, T> {
    /// Create an empty map
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    /// Get tags of an object
    pub fn get(&self, id: &K) -> Option<&HashSet<T>> {
        self.map.get(id)
    }

    /// Get keys of the maps
    pub fn keys(&self) -> Keys<'_, K, HashSet<T>> {
        self.map.keys()
    }

    /// Get keys with given tag
    pub fn tag_keys<'a>(&'a self, tag: &'a T) -> impl Iterator<Item = &K> + 'a {
        self.map.keys().filter(|k| self.tag(k, tag))
    }

    /// Check if an object has the given tag
    pub fn tag(&self, id: &K, tag: &T) -> bool {
        if let Some(tags) = self.map.get(id) {
            tags.contains(tag)
        } else {
            false
        }
    }

    /// Add a tag to an object
    pub fn add(&mut self, id: &K, tag: T) {
        if let Some(tags) = self.map.get_mut(id) {
            tags.insert(tag);
        } else {
            let mut tags = HashSet::new();
            tags.insert(tag);
            self.map.insert(*id, tags);
        }
    }

    /// Remove a tag from an object
    pub fn remove(&mut self, id: &K, tag: &T) {
        if let Some(tags) = self.map.get_mut(id) {
            tags.remove(tag);
            if tags.is_empty() {
                self.map.remove(&id);
            }
        }
    }

    /// Remove an object and its tags from map
    pub fn remove_all(&mut self, id: &K) {
        self.map.remove(id);
    }
}

impl<K: Eq + Hash + Copy + Clone, T: Tag> Default for TagMap<K, T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Tags to be attached on a discord member
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Clone)]
pub enum UserTag {
    NoNickUpdate,
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
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "NoNickUpdate" => Self::NoNickUpdate,
            "NoRoleUpdate" => Self::NoRoleUpdate,
            _ => return Err(io::Error::new(io::ErrorKind::Other, "Failed to convert from str to UserTag")),
        })
    }
}

impl_debug_display!(UserTag);

/// Tags to be attached to a discord channel
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Clone)]
pub enum ChannelTag {
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
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "NoTrack" => Self::NoTrack,
            _ => {
                return Err(io::Error::new(io::ErrorKind::Other, "Failed to convert from str to ChannelTag"))
            }
        })
    }
}

impl_debug_display!(ChannelTag);

/// Tags to be attached to a text channel
#[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Clone)]
pub enum TextChannelTag {
    GuildMemberLog,
    GuildLevelLog,
    XpLog,
    OnlineLog,
}

impl Tag for TextChannelTag {
    fn describe(&self) -> &str {
        match self {
            Self::GuildMemberLog => "Logs guild member join, leave, and rank / ign change",
            Self::GuildLevelLog => "Logs guild level up",
            Self::XpLog => "Logs guild member xp contributions",
            Self::OnlineLog => "Logs player join / leave",
        }
    }
}

impl FromStr for TextChannelTag {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "GuildMemberLog" => Self::GuildMemberLog,
            "GuildLevelLog" => Self::GuildLevelLog,
            "XpLog" => Self::XpLog,
            "OnlineLog" => Self::OnlineLog,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Failed to convert from str to TextChannelTag",
                ))
            }
        })
    }
}

impl_debug_display!(TextChannelTag);
