//! Discord related functions
use std::borrow::Cow;

use anyhow::{Context as AHContext, Result};
use serenity::client::Cache;
use serenity::http::Http;
use serenity::model::channel::{Channel, ChannelCategory, GuildChannel};
use serenity::model::guild::{Guild, Member, Role};
use serenity::model::id::ChannelId;

/// The same as Guild::member_named, but if it returns None,
/// Guild::search_members is used.
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

/// Return a channel's category and parent channel in pair.
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

#[derive(Debug)]
/// Similar to serenity's Channel enum, but without the Private variant
pub enum PublicChannel<'a> {
    Category(&'a ChannelCategory),
    Guild(&'a GuildChannel),
}

impl PublicChannel<'_> {
    /// Get channel's id
    pub fn id(&self) -> ChannelId {
        match self {
            Self::Guild(c) => c.id.clone(),
            Self::Category(c) => c.id.clone(),
        }
    }
}

/// Get a channel or category by id.
/// This function searches in both Guild.channels and Guild.threads
pub fn get_channel<'a>(guild: &'a Guild, id: u64) -> Option<PublicChannel<'a>> {
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

/// Get a channel or category by name.
/// This function searches in both Guild.channels and Guild.threads
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

/// Same as Member.remove_role but accepts Option<&Role>
pub async fn remove_role(http: &Http, role: Option<&Role>, member: &mut Member) -> Result<()> {
    if let Some(role) = role {
        member.remove_role(http, role.id).await.context("Failed to remove role")?
    }
    Ok(())
}

/// Same as Member.add_role but accepts Option<&Role>
pub async fn add_role(http: &Http, role: Option<&Role>, member: &mut Member) -> Result<()> {
    if let Some(role) = role {
        member.add_role(http, role.id).await.context("Failed to add role")?
    }
    Ok(())
}
