use std::borrow::Cow;

use anyhow::{bail, Result};
use reqwest::Client;
use serenity::http::CacheHttp;
use serenity::model::guild::{Guild, Member, Role};
use serenity::model::id::{RoleId, UserId};
use tokio::sync::RwLock;

use memberdb::DB;
use util::discord::PublicChannel;

pub enum TargetObject<'a> {
    Mc(String),
    Discord(DiscordObject<'a>),
}

impl<'a> TargetObject<'a> {
    pub async fn from_hinted(
        cache_http: &'a impl CacheHttp, db: &RwLock<DB>, client: &Client, guild: &'a Guild, s: &'a str,
    ) -> Result<TargetObject<'a>> {
        if let Some((identifier, s)) = s.split_once(':') {
            return Self::parse(cache_http, &db, &client, &guild, identifier, s).await;
        }
        bail!("Invalid format for target string")
    }

    pub async fn from_str(
        cache_http: &'a impl CacheHttp, db: &RwLock<DB>, client: &Client, guild: &'a Guild, s: &'a str,
    ) -> Result<TargetObject<'a>> {
        if let Ok(d_obj) = DiscordObject::parse_ping(cache_http, &guild, &s).await {
            return Ok(Self::Discord(d_obj));
        }
        Self::from_hinted(cache_http, &db, &client, &guild, s).await
    }

    pub async fn parse(
        cache_http: &'a impl CacheHttp, db: &RwLock<DB>, client: &Client, guild: &'a Guild, ident: &str,
        s: &'a str,
    ) -> Result<TargetObject<'a>> {
        match ident {
            "m" => {
                if !wynn::utils::is_valid_ign(s) {
                    bail!("Invalid mc ign")
                }

                let id = {
                    let db = db.read().await;
                    memberdb::get_ign_mcid(&db, s).await?
                };
                match id {
                    Some(id) => Ok(Self::Mc(id)),
                    None => {
                        let id = wynn::get_ign_id(&client, s).await?;
                        Ok(Self::Mc(id))
                    }
                }
            }
            _ => {
                let d_obj = DiscordObject::parse(cache_http, &guild, ident, s).await?;
                Ok(Self::Discord(d_obj))
            }
        }
    }
}

pub enum DiscordObjectType {
    Channel,
    Member,
    Role,
}

pub enum DiscordObject<'a> {
    Channel(PublicChannel<'a>),
    Member(Cow<'a, Member>),
    Role(&'a Role),
}

impl<'a> DiscordObject<'a> {
    pub async fn from_hinted(
        cache_http: &'a impl CacheHttp, guild: &'a Guild, s: &'a str,
    ) -> Result<DiscordObject<'a>> {
        if let Some((identifier, s)) = s.split_once(':') {
            return Self::parse(cache_http, &guild, identifier, s).await;
        }
        bail!("Invalid format for target discord string")
    }

    pub async fn from_str(
        cache_http: &'a impl CacheHttp, guild: &'a Guild, s: &'a str,
    ) -> Result<DiscordObject<'a>> {
        if let Ok(obj) = Self::parse_ping(cache_http, &guild, s).await {
            return Ok(obj);
        }
        Self::from_hinted(cache_http, &guild, s).await
    }

    pub async fn parse_ping(
        cache_http: &impl CacheHttp, guild: &'a Guild, s: &str,
    ) -> Result<DiscordObject<'a>> {
        let (ty, id) = util::some!(extract_id_from_ping(s), bail!("Invalid ping format"));
        match ty {
            DiscordObjectType::Member => {
                return match guild.members.get(&UserId(id)) {
                    Some(member) => Ok(Self::Member(Cow::Borrowed(member))),
                    None => Ok(Self::Member(Cow::Owned(guild.member(&cache_http, &UserId(id)).await?))),
                }
            }
            DiscordObjectType::Channel => {
                let channel_result = util::discord::get_channel(&guild, id);
                if let Some(channel_result) = channel_result {
                    return Ok(Self::Channel(channel_result));
                }
            }
            DiscordObjectType::Role => {
                if let Some(role) = guild.roles.get(&RoleId(id)) {
                    return Ok(Self::Role(role));
                }
            }
        }
        bail!("Failed to get discord object by id")
    }

    pub async fn parse(
        cache_http: &'a impl CacheHttp, guild: &'a Guild, ident: &str, s: &'a str,
    ) -> Result<DiscordObject<'a>> {
        match ident {
            "d" => {
                if let Some(member) = util::discord::get_member_named(&cache_http.http(), &guild, s).await? {
                    return Ok(Self::Member(member));
                }
            }
            "c" => {
                let channel = util::discord::get_channel_named(&guild, s);
                if let Some(channel) = channel {
                    return Ok(Self::Channel(channel));
                }
            }
            "r" => {
                if let Some(role) = guild.role_by_name(s) {
                    return Ok(Self::Role(role));
                }
            }
            _ => {}
        }
        bail!("Unknown discord target identifier")
    }
}

pub fn extract_id_from_ping(ping: &str) -> Option<(DiscordObjectType, u64)> {
    if !ping.starts_with("<") || !ping.ends_with(">") {
        return None;
    }

    // An id is always 18 characters long
    let (ty, id_str) = if ping.starts_with("<#") && ping.len() == 21 {
        (DiscordObjectType::Channel, ping.get(2..20))
    } else if ping.starts_with("<@!") && ping.len() == 22 {
        (DiscordObjectType::Member, ping.get(3..21))
    } else if ping.starts_with("<@&") && ping.len() == 22 {
        (DiscordObjectType::Role, ping.get(3..21))
    } else {
        return None;
    };

    if let Some(Ok(id)) = id_str.map(|s| s.parse()) {
        Some((ty, id))
    } else {
        None
    }
}
