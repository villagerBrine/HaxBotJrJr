//! Discord related functions
use std::borrow::Cow;

use anyhow::{Context as AHContext, Result};
use serenity::client::Cache;
use serenity::http::Http;
use serenity::model::channel::{Channel, ChannelCategory, GuildChannel, PermissionOverwriteType};
use serenity::model::guild::{Guild, Member, Role};
use serenity::model::id::ChannelId;
use serenity::model::permissions::Permissions;

/// Search member in cache using [`Guild::member_named`], but if it isn't cached,
/// [`Guild::search_members`] is used to search over API.
///
/// # Errors
/// Returns [`Error::Http`] if API returns an error.
///
/// [`Guild::member_named`]: serenity::model::guild::Guild::member_named
/// [`Guild::search_members`]: serenity::model::guild::Guild::search_members
/// [`Error::Http`]: serenity::Error::Http
pub async fn get_member_named<'a>(
    http: &'a Http, guild: &'a Guild, name: &'a str,
) -> Result<Option<Cow<'a, Member>>> {
    match guild.member_named(name) {
        Some(member) => Ok(Some(Cow::Borrowed(member))),
        None => {
            let mut members = guild
                .search_members(http, name, Some(1))
                .await
                .context("Failed to search guild members")?;
            if members.is_empty() {
                Ok(None)
            } else {
                let member = members.remove(0);
                Ok(Some(Cow::Owned(member)))
            }
        }
    }
}

/// Return a channel's category and parent channel (if it is a thread) in a tuple of that order.
pub fn get_channel_parents(cache: &Cache, channel: &GuildChannel) -> (Option<ChannelId>, Option<ChannelId>) {
    match channel.parent_id {
        Some(parent_id) => {
            // The parent could be a category, so its parent is also checked.
            if let Some(parent_channel) = cache.guild_channel(parent_id) {
                // If it has a parent, then it is not a category, and its parent is a category.
                if let Some(parent_parent_id) = parent_channel.parent_id {
                    return (Some(parent_parent_id), Some(parent_id));
                }
            }
            // If it has no parent, then it is a category.
            (Some(parent_id), None)
        }
        None => (None, None),
    }
}

/// Similar to [`Channel`], but without the Private variant
///
/// [`Channel`]: serenity::model::channel::Channel
#[derive(Debug, Clone)]
pub enum PublicChannel<'a> {
    /// Guild category
    Category(&'a ChannelCategory),
    /// Guild channel / thread
    Guild(&'a GuildChannel),
}

impl PublicChannel<'_> {
    /// Get channel's id
    pub fn id(&self) -> ChannelId {
        match self {
            Self::Guild(c) => c.id,
            Self::Category(c) => c.id,
        }
    }
}

/// Get cached channel, thread or category by id.
pub fn get_channel(guild: &Guild, id: u64) -> Option<PublicChannel<'_>> {
    match guild.channels.get(&ChannelId(id)) {
        Some(c) => match c {
            Channel::Guild(c) => Some(PublicChannel::Guild(c)),
            Channel::Category(c) => Some(PublicChannel::Category(c)),
            _ => None,
        },
        None => {
            for thread in &guild.threads {
                if thread.id.0 == id {
                    return Some(PublicChannel::Guild(thread));
                }
            }
            None
        }
    }
}

/// Get cached channel, thread or category by name.
pub fn get_channel_named<'a>(guild: &'a Guild, name: &'a str) -> Option<PublicChannel<'a>> {
    for channel in guild.channels.values() {
        match channel {
            Channel::Guild(c) => {
                if c.name == name {
                    return Some(PublicChannel::Guild(c));
                }
            }
            Channel::Category(c) => {
                if c.name == name {
                    return Some(PublicChannel::Category(c));
                }
            }
            _ => continue,
        };
    }
    for thread in &guild.threads {
        if thread.name == name {
            return Some(PublicChannel::Guild(thread));
        }
    }
    None
}

/// Same as [`Member.remove_role`] but accepts `Option<&Role>`.
///
/// # Errors
/// Returns [`Error::Http`] if a role with the given Id does not exist, or if the current user
/// lacks permission.
///
/// [`Error::Http`]: serenity::Error::Http
/// [`Member.remove_role`]: serenity::model::guild::Member::remove_role
pub async fn remove_role_maybe(http: &Http, role: Option<&Role>, member: &mut Member) -> Result<()> {
    if let Some(role) = role {
        member.remove_role(http, role.id).await.context("Failed to remove role")?
    }
    Ok(())
}

/// Same as [`Member.add_role`] but accepts `Option<&Role>`.
///
/// # Errors
/// Returns [`Error::Http`] if a role with the given Id does not exist, or if the current user
/// lacks permission.
///
/// [`Error::Http`]: serenity::Error::Http
/// [`Member.add_role`]: serenity::model::guild::Member::add_role
pub async fn add_role_maybe(http: &Http, role: Option<&Role>, member: &mut Member) -> Result<()> {
    if let Some(role) = role {
        member.add_role(http, role.id).await.context("Failed to add role")?
    }
    Ok(())
}

/// Checks if a guild channel allows a permission.
///
/// If the channel is a thread, then its parent channel is checked.
pub fn check_channel_allow(
    guild: &Guild, channel: &GuildChannel, kind: PermissionOverwriteType, perm: Permissions,
) -> bool {
    // channel is a thread, check parent channel instead
    if channel.thread_metadata.is_some() {
        if let Some(parent_id) = channel.parent_id {
            if let Some(Channel::Guild(parent)) = guild.channels.get(&parent_id) {
                return check_channel_allow(guild, parent, kind, perm);
            }
        }
    }
    for overwrite in &channel.permission_overwrites {
        if overwrite.kind == kind && overwrite.allow == perm {
            return true;
        }
    }
    false
}
