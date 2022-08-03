//! Tools for conversion from string
use std::borrow::Cow;

use anyhow::{bail, Result};
use reqwest::Client;
use serenity::http::CacheHttp;
use serenity::model::guild::{Guild, Member, Role};
use serenity::model::id::{RoleId, UserId};
use tokio::sync::RwLock;

use memberdb::DB;
use util::discord::PublicChannel;
use util::ok;

/// A target is a generalization of an object which the bot can act upon
///
/// # Text representations
/// *Parenthesised texts are to be substituted*
/// General form: `(hint-prefix):(name)`
/// This is referred to as hinted target.
///
/// - Mc account: `m:(ign)`
/// - Discord account: `d:(username)`
/// - Discord role: `r:(name)`
/// - Discord channel: `c:(name)`
///
/// # Pings
/// If a target can be pinged (ex: #general, @user), then its ping can also be parsed into `TargetObject`
/// Note that the example pings are not what is actually being parsed, but instead their textual
/// form, ex: <#2783764387>
#[derive(Debug)]
pub enum TargetObject<'a> {
    Mc(String),
    Discord(DiscordObject<'a>),
}

impl<'a> TargetObject<'a> {
    /// Parse hinted target string
    pub async fn from_hinted(
        cache_http: &'a impl CacheHttp, db: &RwLock<DB>, client: &Client, guild: &'a Guild, s: &'a str,
    ) -> Result<TargetObject<'a>> {
        if s.is_empty() {
            bail!("Target is empty")
        }
        if let Some((prefix, name)) = s.split_once(':') {
            return Self::parse(cache_http, &db, client, guild, prefix, name).await;
        }
        bail!("Invalid format")
    }

    /// Parse bot hinted and pinged target string
    pub async fn from_str(
        cache_http: &'a impl CacheHttp, db: &RwLock<DB>, client: &Client, guild: &'a Guild, s: &'a str,
    ) -> Result<TargetObject<'a>> {
        if s.is_empty() {
            bail!("Target is empty")
        }
        // Ping parsing is prioritized
        if let Ok(d_obj) = DiscordObject::parse_ping(cache_http, guild, s).await {
            return Ok(Self::Discord(d_obj));
        }
        Self::from_hinted(cache_http, &db, client, guild, s).await
    }

    /// Parse hinted target components: its prefix and target name.
    pub async fn parse(
        cache_http: &'a impl CacheHttp, db: &RwLock<DB>, client: &Client, guild: &'a Guild, prefix: &str,
        name: &'a str,
    ) -> Result<TargetObject<'a>> {
        match prefix {
            "m" => {
                if !wynn::utils::is_valid_ign(name) {
                    bail!("Invalid mc ign")
                }

                // Tries to get mcid from database first, if fails, then mojang api is used
                let id = {
                    let db = db.read().await;
                    memberdb::get_ign_mcid(&db, name).await?
                };
                match id {
                    Some(id) => Ok(Self::Mc(id)),
                    None => {
                        let id = ok!(
                            wynn::get_ign_id(client, name).await,
                            bail!("Failed to find player with given ign")
                        );
                        Ok(Self::Mc(id))
                    }
                }
            }
            _ => {
                let d_obj = DiscordObject::parse(cache_http, guild, prefix, name).await?;
                Ok(Self::Discord(d_obj))
            }
        }
    }
}

/// Types of discord objects, directly corresponds to `DiscordObject`
#[derive(Debug)]
pub enum DiscordObjectType {
    Channel,
    Member,
    Role,
}

/// Subset of `TargetObject`
#[derive(Debug)]
pub enum DiscordObject<'a> {
    Channel(PublicChannel<'a>),
    Member(Cow<'a, Member>),
    Role(&'a Role),
}

impl<'a> DiscordObject<'a> {
    /// Parse hinted discord target string
    pub async fn from_hinted(
        cache_http: &'a impl CacheHttp, guild: &'a Guild, s: &'a str,
    ) -> Result<DiscordObject<'a>> {
        if s.is_empty() {
            bail!("Target is empty")
        }
        if let Some((identifier, s)) = s.split_once(':') {
            return Self::parse(cache_http, guild, identifier, s).await;
        }
        bail!("Invalid format")
    }

    /// Parse both hinted and pinged discord target string
    pub async fn from_str(
        cache_http: &'a impl CacheHttp, guild: &'a Guild, s: &'a str,
    ) -> Result<DiscordObject<'a>> {
        if s.is_empty() {
            bail!("Target is empty")
        }
        // Ping parsing is prioritized
        if let Ok(obj) = Self::parse_ping(cache_http, guild, s).await {
            return Ok(obj);
        }
        Self::from_hinted(cache_http, guild, s).await
    }

    /// Parse pinged discord target string
    pub async fn parse_ping(
        cache_http: &impl CacheHttp, guild: &'a Guild, s: &str,
    ) -> Result<DiscordObject<'a>> {
        let (ty, id) = util::some!(extract_id_from_ping(s), bail!("Invalid ping format"));
        match ty {
            DiscordObjectType::Member => {
                // Tries to find member from cache, if failed, fetch it over api
                return match guild.members.get(&UserId(id)) {
                    Some(member) => Ok(Self::Member(Cow::Borrowed(member))),
                    None => Ok(Self::Member(Cow::Owned(guild.member(cache_http, &UserId(id)).await?))),
                };
            }
            DiscordObjectType::Channel => {
                let channel_result = util::discord::get_channel(guild, id);
                if let Some(channel_result) = channel_result {
                    return Ok(Self::Channel(channel_result));
                }
                bail!("Failed to find linked channel")
            }
            DiscordObjectType::Role => {
                if let Some(role) = guild.roles.get(&RoleId(id)) {
                    return Ok(Self::Role(role));
                }
                bail!("Failed to find pinged role")
            }
        }
    }

    /// Parse hinted discord target components: its prefix and target name.
    pub async fn parse(
        cache_http: &'a impl CacheHttp, guild: &'a Guild, ident: &str, s: &'a str,
    ) -> Result<DiscordObject<'a>> {
        match ident {
            "d" => {
                if let Some(member) = util::discord::get_member_named(&cache_http.http(), guild, s).await? {
                    return Ok(Self::Member(member));
                }
                bail!("Failed to find member with given name")
            }
            "c" => {
                let channel = util::discord::get_channel_named(guild, s);
                if let Some(channel) = channel {
                    return Ok(Self::Channel(channel));
                }
                bail!("Failed to find channel/category with given name")
            }
            "r" => {
                if let Some(role) = guild.role_by_name(s) {
                    return Ok(Self::Role(role));
                }
                bail!("Failed to find role with given name")
            }
            _ => {}
        }
        bail!("Unknown target identifier")
    }
}

/// Given a discord ping string (ex: #general, @user), determine its type and extract out contained
/// id.
/// Note that the example pings are not what this function actually accepts, but instead their
/// textual form, ex: <#23746372838>
pub fn extract_id_from_ping(ping: &str) -> Option<(DiscordObjectType, u64)> {
    if !ping.starts_with("<") || !ping.ends_with(">") {
        return None;
    }

    // An discord id is always 18 characters long, except for thread id, it is 19 characters long

    // Channel ping format: <#id>
    let (ty, id_str) = if ping.starts_with("<#") {
        if ping.len() == 21 {
            (DiscordObjectType::Channel, ping.get(2..20))
        } else {
            (DiscordObjectType::Channel, ping.get(2..21))
        }
    // User ping format: <@!id>
    } else if ping.starts_with("<@!") && ping.len() == 22 {
        (DiscordObjectType::Member, ping.get(3..21))
    // Role ping format: <@&id>
    } else if ping.starts_with("<@&") && ping.len() == 22 {
        (DiscordObjectType::Role, ping.get(3..21))
    // ALternative user ping format: <@id>
    } else if ping.starts_with("<@") && ping.len() == 21 {
        (DiscordObjectType::Member, ping.get(2..20))
    } else {
        return None;
    };

    if let Some(Ok(id)) = id_str.map(|s| s.parse()) {
        Some((ty, id))
    } else {
        None
    }
}
