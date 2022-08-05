//! API response models, type aliases, and useful constants
//!
//! TODO: Make the string fields be borrowed instead of owned
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
pub struct MojangIgnIdResponse {
    pub id: String,
    pub name: String,
}

pub type MojangIgnResponse = Vec<MojangIgnResponseItem>;

#[derive(Debug, Deserialize, Clone)]
pub struct MojangIgnResponseItem {
    pub name: String,
    #[serde(rename = "changedToAt", default)]
    pub changed_to_at: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RequestInfo {
    pub timestamp: u64,
    pub version: u8,
}

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

pub type ServerList = HashMap<String, serde_json::Value>;

pub const IG_RANKS: [&str; 6] = ["OWNER", "CHIEF", "STRATEGIST", "CAPTAIN", "RECRUITER", "RECRUIT"];
