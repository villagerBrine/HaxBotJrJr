//! Tools for conversion from string
//!
//! This module provides [`TargetObject`] for general conversion, and [`DiscordObject`] for discord
//! only general conversion.
//!
//! If you want ping only general conversion, use [`extract_id_from_ping`].
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
/// This struct is used for general string parsing, allow you to parse a string into either a mcid,
/// or a discord object.
///
/// [`TargetObject`] provides two static methods for parsing, [`TargetObject.from_hinted`] parses
/// hinted target, and [`TargetObject.from_str`] parses from both hinted target and ping.
///
/// # Hinted target
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
    Discord(Box<DiscordObject<'a>>),
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
            return Self::parse(cache_http, db, client, guild, prefix, name).await;
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
        if let Ok(d_obj) = DiscordObject::from_ping(cache_http, guild, s).await {
            return Ok(Self::Discord(Box::new(d_obj)));
        }
        Self::from_hinted(cache_http, db, client, guild, s).await
    }

    /// Parse hinted target components: its prefix and target name.
    async fn parse(
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
                    memberdb::get_ign_mcid(&mut db.exe(), name).await?
                };
                match id {
                    Some(id) => Ok(Self::Mc(id)),
                    None => {
                        let id = ok!(
                            wynn::get_id(client, name).await,
                            bail!("Failed to find player with given ign")
                        );
                        Ok(Self::Mc(id))
                    }
                }
            }
            _ => {
                let d_obj = DiscordObject::parse(cache_http, guild, prefix, name).await?;
                Ok(Self::Discord(Box::new(d_obj)))
            }
        }
    }
}

/// Types of discord objects, directly corresponds to [`DiscordObject`]
#[derive(Debug, PartialEq, Eq)]
pub enum DiscordObjectType {
    Channel,
    Member,
    Role,
}

/// Generalization of a discord object which the bot can act upon
///
/// Subset of [`TargetObject`].
///
/// This struct is used for general string parsing to a discord object, for general parsing that
/// also includes mc account, use [`TargetObject`].
///
/// [`DiscordObject`] provides three static methods for parsing, [`DiscordObject.from_hinted`] parses
/// hinted target, [`DiscordObject.from_ping`] parses discord ping, and [`DiscordObject.from_str`]
/// parses both hinted target and discord ping.
///
/// # Hinted target
/// *Parenthesised texts are to be substituted*
/// General form: `(hint-prefix):(name)`
/// This is referred to as hinted target.
///
/// - Discord account: `d:(username)`
/// - Discord role: `r:(name)`
/// - Discord channel: `c:(name)`
///
/// # Pings
/// If a target can be pinged (ex: #general, @user), then its ping can also be parsed into `TargetObject`
/// Note that the example pings are not what is actually being parsed, but instead their textual
/// form, ex: <#2783764387>
#[derive(Debug)]
pub enum DiscordObject<'a> {
    Channel(PublicChannel<'a>),
    Member(Box<Cow<'a, Member>>),
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
        if let Ok(obj) = Self::from_ping(cache_http, guild, s).await {
            return Ok(obj);
        }
        Self::from_hinted(cache_http, guild, s).await
    }

    /// Parse pinged discord target string
    pub async fn from_ping(
        cache_http: &impl CacheHttp, guild: &'a Guild, s: &str,
    ) -> Result<DiscordObject<'a>> {
        let (ty, id) = util::some!(extract_id_from_ping(s), bail!("Invalid ping format"));
        match ty {
            DiscordObjectType::Member => {
                // Tries to find member from cache, if failed, fetch it over api
                return match guild.members.get(&UserId(id)) {
                    Some(member) => Ok(Self::Member(Box::new(Cow::Borrowed(member)))),
                    None => {
                        let member = guild.member(cache_http, &UserId(id)).await?;
                        Ok(Self::Member(Box::new(Cow::Owned(member))))
                    }
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
    async fn parse(
        cache_http: &'a impl CacheHttp, guild: &'a Guild, ident: &str, s: &'a str,
    ) -> Result<DiscordObject<'a>> {
        match ident {
            "d" => {
                if let Some(member) = util::discord::get_member_named(cache_http.http(), guild, s).await? {
                    return Ok(Self::Member(Box::new(member)));
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

/// Extract id from discord ping.
///
/// Given a discord ping string (ex: #general, @user), determine its type and extract out contained
/// id.
/// Note that the example pings are not what this function actually accepts, but instead their
/// textual form, ex: <#23746372838>
/// ```
/// use msgtool::parser::{DiscordObjectType, extract_id_from_ping};
///
/// assert!(extract_id_from_ping("<#740381703201226883>") ==
///         Some((DiscordObjectType::Channel, 740381703201226883)));
/// assert!(extract_id_from_ping("<#1001916835530277056>") ==
///         Some((DiscordObjectType::Channel, 1001916835530277056)));
/// assert!(extract_id_from_ping("<@658478931682394134>") ==
///         Some((DiscordObjectType::Member, 658478931682394134)));
/// assert!(extract_id_from_ping("<@!450506596582162442>") ==
///         Some((DiscordObjectType::Member, 450506596582162442)));
/// assert!(extract_id_from_ping("<@&440261512041332746>") ==
///         Some((DiscordObjectType::Role, 440261512041332746)));
/// assert!(extract_id_from_ping("<@&1234>").is_none());
/// assert!(extract_id_from_ping("<$%440261512041332746>").is_none());
/// ```
pub fn extract_id_from_ping(ping: &str) -> Option<(DiscordObjectType, u64)> {
    if !ping.starts_with('<') || !ping.ends_with('>') {
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
