//! Functions for fetching data from the database
use std::str::FromStr;

use anyhow::{Context, Result};
use sqlx::{query, query_as};

use crate::model::discord::{DiscordId, DiscordProfile};
use crate::model::guild::{GuildProfile, GuildProfileRow, GuildRank};
use crate::model::member::{Member, MemberId, MemberRank, MemberRow, MemberType};
use crate::model::wynn::{McId, WynnProfile, WynnProfileRow};
use crate::Executor;

/// Get member from database
pub async fn get_member(exe: &mut Executor<'_>, mid: MemberId) -> Result<Option<Member>> {
    let member = exe
        .optional(query_as!(
            MemberRow,
            "SELECT oid AS id,type AS member_type,discord,mcid,rank FROM member WHERE oid=?",
            mid
        ))
        .await
        .context("Failed to fetch member")?;
    match member {
        Some(member) => Ok(Some(Member::from_row(member)?)),
        None => Ok(None),
    }
}

/// Given a discord id, return that profile's linked member id if there is any.
pub async fn get_discord_mid(exe: &mut Executor<'_>, discord_id: DiscordId) -> Result<Option<MemberId>> {
    let row = exe
        .optional(query!("SELECT mid FROM discord WHERE id=?", discord_id))
        .await
        .context("Failed to fetch mid from discord table")?;
    if let Some(row) = row {
        if let Some(mid) = row.mid {
            return Ok(Some(mid));
        }
    }
    Ok(None)
}

/// Fetch wynn profile of given id (assuming it exists).
pub async fn get_wynn_mid(exe: &mut Executor<'_>, mcid: &str) -> Result<Option<MemberId>> {
    let row = exe
        .optional(query!("SELECT mid FROM wynn WHERE id=?", mcid))
        .await
        .context("Failed to fetch mid from wynn table")?;
    if let Some(row) = row {
        if let Some(mid) = row.mid {
            return Ok(Some(mid));
        }
    }
    Ok(None)
}

/// Fetch the member id that is linked to a wynn profile(assuming the profile exists).
pub async fn get_ign_mid(exe: &mut Executor<'_>, ign: &str) -> Result<Option<MemberId>> {
    Ok(exe
        .one(query!("SELECT mid FROM wynn WHERE ign=?", ign))
        .await
        .context("Failed to get wynn.mid")?
        .mid)
}

/// Fetch the ign of a wynn profile (assuming the profile exists).
pub async fn get_ign(exe: &mut Executor<'_>, mcid: &str) -> Result<String> {
    Ok(exe
        .one(query!("SELECT ign FROM wynn WHERE id=?", mcid))
        .await
        .context("Failed to get wynn.ign")?
        .ign)
}

/// Check a member of given id exists
pub async fn member_exist(exe: &mut Executor<'_>, mid: MemberId) -> Result<bool> {
    Ok(exe
        .optional(query!("SELECT oid FROM member WHERE oid=?", mid))
        .await
        .context("Failed to fetch from member table")?
        .is_some())
}

/// Get a member's discord and mc ids
pub async fn get_member_links(
    exe: &mut Executor<'_>, mid: MemberId,
) -> Result<(Option<DiscordId>, Option<McId>)> {
    let row = exe
        .one(query!("SELECT discord,mcid FROM member WHERE oid=?", mid))
        .await
        .context("Failed to fetch from member table")?;
    Ok((row.discord, row.mcid))
}

/// Get a member's type
pub async fn get_member_type(exe: &mut Executor<'_>, mid: MemberId) -> Result<MemberType> {
    let row = exe
        .one(query!("SELECT type AS member_type FROM member WHERE oid=?", mid))
        .await
        .context("Failed to fetch member.type")?;
    Ok(MemberType::from_str(&row.member_type)?)
}

/// Get a member's rank
pub async fn get_member_rank(exe: &mut Executor<'_>, mid: MemberId) -> Result<MemberRank> {
    let rank = exe
        .one(query!("SELECT rank FROM member WHERE oid=?", mid))
        .await
        .context("Failed to get member.rank")?
        .rank;
    Ok(MemberRank::decode(&rank)?)
}

/// Get discord profile from database.
pub async fn get_discord_profile(
    exe: &mut Executor<'_>, discord_id: DiscordId,
) -> Result<Option<DiscordProfile>> {
    let discord = exe
        .optional(query_as!(DiscordProfile, "SELECT * FROM discord WHERE id=?", discord_id))
        .await
        .context("Failed to select from discord table")?;
    Ok(discord)
}

/// If a discord profile of given id exists.
pub async fn discord_profile_exist(exe: &mut Executor<'_>, discord_id: DiscordId) -> Result<bool> {
    Ok(exe
        .optional(query!("SELECT id FROM discord WHERE id=?", discord_id))
        .await
        .context("Failed to fetch from discord table")?
        .is_some())
}

/// Get wynn profile from database.
pub async fn get_wynn_profile(exe: &mut Executor<'_>, mcid: &str) -> Result<Option<WynnProfile>> {
    let wynn = exe
        .optional(query_as!(WynnProfileRow, "SELECT * FROM wynn WHERE id=?", mcid))
        .await
        .context("Failed to fetch from wynn table")?;
    Ok(wynn.map(|row| WynnProfile::from_row(row)))
}

/// If a wynn profile of given id exists.
pub async fn wynn_profile_exist(exe: &mut Executor<'_>, mcid: &str) -> Result<bool> {
    Ok(exe
        .optional(query!("SELECT id FROM wynn WHERE id=?", mcid))
        .await
        .context("Failed to fetch from wynn table")?
        .is_some())
}

/// Get the id of a wynn profile with given ign.
pub async fn get_ign_mcid(exe: &mut Executor<'_>, ign: &str) -> Result<Option<McId>> {
    let row = exe
        .optional(query!("SELECT id FROM wynn WHERE ign=?", ign))
        .await
        .context("Failed to fetch wynn.id")?;

    match row {
        Some(row) => Ok(Some(row.id)),
        None => Ok(None),
    }
}

/// Check if a mc id is in a guild
pub async fn is_in_guild(exe: &mut Executor<'_>, mcid: &str) -> Result<bool> {
    Ok(exe
        .optional(query!("SELECT guild FROM wynn WHERE id=?", mcid))
        .await
        .context("Failed to fetch wynn.guild")?
        .map(|n| n.guild > 0)
        .unwrap_or(false))
}

/// Get guild profile from database.
pub async fn get_guild_profile(exe: &mut Executor<'_>, mcid: &str) -> Result<Option<GuildProfile>> {
    let guild = exe
        .optional(query_as!(GuildProfileRow, "SELECT * FROM guild WHERE id=?", mcid))
        .await
        .context("Failed to fetch from guild table")?;
    match guild {
        Some(guild) => Ok(Some(GuildProfile::from_row(exe, guild).await?)),
        None => Ok(None),
    }
}

/// If a guild profile of given id exists.
pub async fn guild_profile_exist(exe: &mut Executor<'_>, mcid: &str) -> Result<bool> {
    Ok(exe
        .optional(query!("SELECT id FROM guild WHERE id=?", mcid))
        .await
        .context("Failed to select from guild table")?
        .is_some())
}

/// Get a guild profile's guild rank (assuming the profile exists).
pub async fn get_guild_rank(exe: &mut Executor<'_>, mcid: &str) -> Result<GuildRank> {
    let rank = exe
        .one(query!("SELECT rank FROM guild WHERE id=?", mcid))
        .await
        .context("Failed to get guild.rank")?
        .rank;
    Ok(GuildRank::from_str(&rank)?)
}

/// Get a guild profile's total xp (assuming the profile exists).
pub async fn get_xp(exe: &mut Executor<'_>, mcid: &str) -> Result<i64> {
    Ok(exe
        .one(query!("SELECT xp FROM guild WHERE id=?", mcid))
        .await
        .context("Failed to get guild.xp")?
        .xp)
}
