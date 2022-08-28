//! Wynncraft and Mojang API response models, and useful constants
//!
//! TODO: Make the string fields be borrowed instead of owned
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// API response of "api.mojang.com/users/profiles/minecraft/"
#[derive(Debug, Deserialize, Clone)]
pub struct MojangIdResponse {
    pub id: String,
    pub name: String,
}

/// API response of "api.mojang.com/user/profiles/.../names"
pub type MojangIgnResponse = Vec<MojangIgnResponseItem>;

/// Model of list item in [`MojangIgnResponse`]
#[derive(Debug, Deserialize, Clone)]
pub struct MojangIgnResponseItem {
    pub name: String,
    #[serde(rename = "changedToAt", default)]
    pub changed_to_at: u64,
}

/// The request info object that is included in the wynncraft API response
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestInfo {
    pub timestamp: u64,
    pub version: u8,
}

/// API response of "api.wynncraft.com/public_api.php?action=guildStats&command=..."
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Guild {
    pub name: String,
    pub prefix: String,
    pub members: Vec<GuildMember>,
    pub xp: f32,
    pub level: u8,
    pub created: String,
    #[serde(rename = "createdFriendly")]
    pub created_friendly: String,
    pub territories: u16,
    pub request: RequestInfo,
}

/// Guild member object from wynncraft API response
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GuildMember {
    pub name: String,
    pub uuid: String,
    pub rank: String,
    pub contributed: i64,
    pub joined: String,
    #[serde(rename = "joinedFriendly")]
    pub joined_friendly: String,
}

/// API response of "api.wynncraft.com/public_api.php?action=onlinePlayers".
///
/// The reason this is modeled using [`serde_json::Value`] is due to inconsistent map value, more
/// specifically the [`RequestInfo`] object is mixed within the player lists.
pub type ServerList = HashMap<String, serde_json::Value>;

/// List of in-game guild ranks, as appeared in wynncraft API response.
/// Rank ordered in descending order
pub const IG_RANKS: [&str; 6] = ["OWNER", "CHIEF", "STRATEGIST", "CAPTAIN", "RECRUITER", "RECRUIT"];
