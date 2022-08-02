//! Functions for fetching data from the database
//!
//! Some function have a *_tx counterpart that accepts `Transaction` instead of `DB`. I know there
//! are ways to make the functions able to work with both, but this is the easiest way and it
//! works.
use std::str::FromStr;

use anyhow::{Context, Result};
use sqlx::{query, query_as};

use crate::model::discord::{DiscordId, DiscordProfile};
use crate::model::guild::{GuildProfile, GuildProfileRow, GuildRank};
use crate::model::member::{Member, MemberId, MemberRank, MemberRow, MemberType};
use crate::model::wynn::{McId, WynnProfile, WynnProfileRow};
use crate::{Transaction, DB};

/// Get member from database
pub async fn get_member(db: &DB, mid: MemberId) -> Result<Option<Member>> {
    let member = query_as!(
        MemberRow,
        "SELECT oid AS id,type AS member_type,discord,mcid,rank FROM member WHERE oid=?",
        mid
    )
    .fetch_optional(&db.pool)
    .await
    .context("Failed to fetch member")?;
    match member {
        Some(member) => Ok(Some(Member::from_row(member)?)),
        None => Ok(None),
    }
}

/// Given a discord id, return that profile's linked member id if there is any.
pub async fn get_discord_mid(db: &DB, discord_id: DiscordId) -> Result<Option<MemberId>> {
    let row = query!("SELECT mid FROM discord WHERE id=?", discord_id)
        .fetch_optional(&db.pool)
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
pub async fn get_wynn_mid(db: &DB, mcid: &str) -> Result<Option<MemberId>> {
    let row = query!("SELECT mid FROM wynn WHERE id=?", mcid)
        .fetch_optional(&db.pool)
        .await
        .context("Failed to fetch mid from wynn table")?;
    if let Some(row) = row {
        if let Some(mid) = row.mid {
            return Ok(Some(mid));
        }
    }
    Ok(None)
}

/// Fetch wynn profile of given id (assuming it exists).
pub async fn get_wynn_mid_tx(tx: &mut Transaction, mcid: &str) -> Result<Option<MemberId>> {
    let row = query!("SELECT mid FROM wynn WHERE id=?", mcid)
        .fetch_optional(&mut tx.tx)
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
pub async fn get_ign_mid(db: &DB, ign: &str) -> Result<Option<MemberId>> {
    Ok(query!("SELECT mid FROM wynn WHERE ign=?", ign)
        .fetch_one(&db.pool)
        .await
        .context("Failed to get wynn.mid")?
        .mid)
}

/// Fetch the ign of a wynn profile (assuming the profile exists).
pub async fn get_ign(db: &DB, mcid: &str) -> Result<String> {
    Ok(query!("SELECT ign FROM wynn WHERE id=?", mcid)
        .fetch_one(&db.pool)
        .await
        .context("Failed to get wynn.ign")?
        .ign)
}

/// Check a member of given id exists
pub async fn member_exist(db: &DB, mid: MemberId) -> Result<bool> {
    Ok(query!("SELECT oid FROM member WHERE oid=?", mid)
        .fetch_optional(&db.pool)
        .await
        .context("Failed to fetch from member table")?
        .is_some())
}

/// Get a member's discord and mc ids
pub async fn get_member_links(db: &DB, mid: MemberId) -> Result<(Option<DiscordId>, Option<McId>)> {
    let row = query!("SELECT discord,mcid FROM member WHERE oid=?", mid)
        .fetch_one(&db.pool)
        .await
        .context("Failed to fetch from member table")?;
    Ok((row.discord, row.mcid))
}

/// Get a member's discord and mc ids
pub async fn get_member_links_tx(
    tx: &mut Transaction, mid: MemberId,
) -> Result<(Option<DiscordId>, Option<McId>)> {
    let row = query!("SELECT discord,mcid FROM member WHERE oid=?", mid)
        .fetch_one(&mut tx.tx)
        .await
        .context("Failed to fetch from member table")?;
    Ok((row.discord, row.mcid))
}

/// Get a member's type
pub async fn get_member_type(db: &DB, mid: MemberId) -> Result<MemberType> {
    let row = query!("SELECT type AS member_type FROM member WHERE oid=?", mid)
        .fetch_one(&db.pool)
        .await
        .context("Failed to fetch member.type")?;
    Ok(MemberType::from_str(&row.member_type)?)
}

/// Get a member's type
pub async fn get_member_type_tx(tx: &mut Transaction, mid: MemberId) -> Result<MemberType> {
    let row = query!("SELECT type AS member_type FROM member WHERE oid=?", mid)
        .fetch_one(&mut tx.tx)
        .await
        .context("Failed to fetch member.type")?;
    Ok(MemberType::from_str(&row.member_type)?)
}

/// Get a member's rank
pub async fn get_member_rank(db: &DB, mid: MemberId) -> Result<MemberRank> {
    let rank = query!("SELECT rank FROM member WHERE oid=?", mid)
        .fetch_one(&db.pool)
        .await
        .context("Failed to get member.rank")?
        .rank;
    Ok(MemberRank::decode(&rank)?)
}

/// Get discord profile from database.
pub async fn get_discord_profile(db: &DB, discord_id: DiscordId) -> Result<Option<DiscordProfile>> {
    let discord = query_as!(DiscordProfile, "SELECT * FROM discord WHERE id=?", discord_id)
        .fetch_optional(&db.pool)
        .await
        .context("Failed to select from discord table")?;
    Ok(discord)
}

/// If a discord profile of given id exists.
pub async fn discord_profile_exist(db: &DB, discord_id: DiscordId) -> Result<bool> {
    Ok(query!("SELECT id FROM discord WHERE id=?", discord_id)
        .fetch_optional(&db.pool)
        .await
        .context("Failed to fetch from discord table")?
        .is_some())
}

/// If a discord profile of given id exists.
pub async fn discord_profile_exist_tx(tx: &mut Transaction, discord_id: DiscordId) -> Result<bool> {
    Ok(query!("SELECT id FROM discord WHERE id=?", discord_id)
        .fetch_optional(&mut tx.tx)
        .await
        .context("Failed to fetch from discord table")?
        .is_some())
}

/// Get wynn profile from database.
pub async fn get_wynn_profile(db: &DB, mcid: &str) -> Result<Option<WynnProfile>> {
    let wynn = query_as!(WynnProfileRow, "SELECT * FROM wynn WHERE id=?", mcid)
        .fetch_optional(&db.pool)
        .await
        .context("Failed to fetch from wynn table")?;
    Ok(wynn.map(|row| WynnProfile::from_row(row)))
}

/// If a wynn profile of given id exists.
pub async fn wynn_profile_exist(db: &DB, mcid: &str) -> Result<bool> {
    Ok(query!("SELECT id FROM wynn WHERE id=?", mcid)
        .fetch_optional(&db.pool)
        .await
        .context("Failed to fetch from wynn table")?
        .is_some())
}

/// If a wynn profile of given id exists.
pub async fn wynn_profile_exist_tx(tx: &mut Transaction, mcid: &str) -> Result<bool> {
    Ok(query!("SELECT id FROM wynn WHERE id=?", mcid)
        .fetch_optional(&mut tx.tx)
        .await
        .context("Failed to fetch from wynn table")?
        .is_some())
}

/// Get the id of a wynn profile with given ign.
pub async fn get_ign_mcid(db: &DB, ign: &str) -> Result<Option<McId>> {
    let row = query!("SELECT id FROM wynn WHERE ign=?", ign)
        .fetch_optional(&db.pool)
        .await
        .context("Failed to fetch wynn.id")?;

    match row {
        Some(row) => Ok(Some(row.id)),
        None => Ok(None),
    }
}

/// Check if a mc id is in a guild
pub async fn is_in_guild(db: &DB, mcid: &str) -> Result<bool> {
    Ok(query!("SELECT guild FROM wynn WHERE id=?", mcid)
        .fetch_optional(&db.pool)
        .await
        .context("Failed to fetch wynn.guild")?
        .map(|n| n.guild > 0)
        .unwrap_or(false))
}

/// Check if a mc id is in a guild
pub async fn is_in_guild_tx(tx: &mut Transaction, mcid: &str) -> Result<bool> {
    Ok(query!("SELECT guild FROM wynn WHERE id=?", mcid)
        .fetch_optional(&mut tx.tx)
        .await
        .context("Failed to fetch wynn.guild")?
        .map(|n| n.guild > 0)
        .unwrap_or(false))
}

/// Get guild profile from database.
pub async fn get_guild_profile(db: &DB, mcid: &str) -> Result<Option<GuildProfile>> {
    let guild = query_as!(GuildProfileRow, "SELECT * FROM guild WHERE id=?", mcid)
        .fetch_optional(&db.pool)
        .await
        .context("Failed to fetch from guild table")?;
    match guild {
        Some(guild) => Ok(Some(GuildProfile::from_row(&db, guild).await?)),
        None => Ok(None),
    }
}

/// If a guild profile of given id exists.
pub async fn guild_profile_exist(db: &DB, mcid: &str) -> Result<bool> {
    Ok(query!("SELECT id FROM guild WHERE id=?", mcid)
        .fetch_optional(&db.pool)
        .await
        .context("Failed to select from guild table")?
        .is_some())
}

/// If a guild profile of given id exists.
pub async fn guild_profile_exist_tx(tx: &mut Transaction, mcid: &str) -> Result<bool> {
    Ok(query!("SELECT id FROM guild WHERE id=?", mcid)
        .fetch_optional(&mut tx.tx)
        .await
        .context("Failed to select from guild table")?
        .is_some())
}

/// Get a guild profile's guild rank (assuming the profile exists).
pub async fn get_guild_rank(db: &DB, mcid: &str) -> Result<GuildRank> {
    let rank = query!("SELECT rank FROM guild WHERE id=?", mcid)
        .fetch_one(&db.pool)
        .await
        .context("Failed to get guild.rank")?
        .rank;
    Ok(GuildRank::from_str(&rank)?)
}

/// Get a guild profile's total xp (assuming the profile exists).
pub async fn get_xp(db: &DB, mcid: &str) -> Result<i64> {
    Ok(query!("SELECT xp FROM guild WHERE id=?", mcid)
        .fetch_one(&db.pool)
        .await
        .context("Failed to get guild.xp")?
        .xp)
}
