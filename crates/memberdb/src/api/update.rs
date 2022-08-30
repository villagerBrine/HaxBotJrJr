//! Functions that modifies the database.
//!
//! For the database to be integrate, following needs to be true:
//! 1. **No dangling profile links**
//!    A member's profile link has to correspond to an existing profile.
//! 2. **No dangling member links**
//!    A profile's member link has to correspond to an existing member.
//! 3. **Closed links**
//!    Member and its profiles has to be linked to each others, forming a closed structure.
//! 4. **Guild-wynn profile relation**
//!    All guild profiles needs to also have a corresponding wynn profile, and those wynn profiles
//!    need to also indicates that they are in guild via `wynn.guild`.
//! 5. **Member type**
//!    A member's type needs to correctly describe its linked profiles.
//! 6. **No empty member**
//!    Any member that doesn't have any profiles linked are to be deleted.
//!
//! The functions assumes the integrity of the database as a precondition.
//! The functions only attempts to preserve the database integrity **AFTER** the modification, it
//! won't attempts to check if that modification is valid.
//! You need to perform these checks yourself as outlined in the function preconditions, this is to
//! prevent redundant checks.
use anyhow::{Context, Result};
use serenity::client::Cache;
use sqlx::query;
use tracing::{info, instrument, warn};

use util::ctx;

use crate::api::fetch::*;
use crate::events::DBEvent;
use crate::model::discord::DiscordId;
use crate::model::guild::GuildRank;
use crate::model::member::{MemberId, MemberRank, MemberType};
use crate::model::wynn::McId;
use crate::query::Stat;
use crate::{Transaction, DB};

/// Add discord partial member, if profile doesn't exist, it is created.
///
/// # Preconditions
/// The given discord id is unlinked
#[instrument(skip(tx))]
pub async fn add_member_discord(
    tx: &mut Transaction, discord_id: DiscordId, rank: MemberRank,
) -> Result<MemberId> {
    info!("Adding discord partial member into database");
    let mid = query!(
        "INSERT INTO member (discord,type,rank) VALUES (?,?,?)",
        discord_id,
        MemberType::DiscordPartial,
        rank
    )
    .execute(&mut tx.tx)
    .await
    .context("Failed to add discord partial member to database")?
    .last_insert_rowid();

    link_or_add_discord(tx, Some(mid), discord_id).await?;

    tx.signal(DBEvent::MemberAdd {
        mid,
        discord_id: Some(discord_id),
        mcid: None,
        rank,
    });
    Ok(mid)
}

/// Add wynn partial member, if profile doesn't exist, it is created.
///
/// # Preconditions
/// The given mcid is unlinked
#[instrument(skip(tx))]
pub async fn add_member_wynn(
    tx: &mut Transaction, mcid: &str, rank: MemberRank, ign: &str,
) -> Result<MemberId> {
    info!("Adding wynn partial member into database");
    let mid =
        query!("INSERT INTO member (mcid,type,rank) VALUES (?,?,?)", mcid, MemberType::WynnPartial, rank)
            .execute(&mut tx.tx)
            .await
            .context("Failed to add wynn partial member")?
            .last_insert_rowid();

    link_or_add_wynn(tx, Some(mid), mcid, ign).await?;

    tx.signal(DBEvent::MemberAdd {
        mid,
        discord_id: None,
        mcid: Some(mcid.to_string()),
        rank,
    });
    Ok(mid)
}

/// Add full member, if any profiles doesn't exist, it is created.
///
/// # Preconditions
/// Both the given mcid and discord id are unlinked
#[instrument(skip(tx))]
pub async fn add_member(
    tx: &mut Transaction, discord_id: DiscordId, mcid: &str, ign: &str, rank: MemberRank,
) -> Result<MemberId> {
    info!("Adding full member into database");
    let mid = query!(
        "INSERT INTO member (discord,mcid,type,rank) VALUES (?,?,?,?)",
        discord_id,
        mcid,
        MemberType::Full,
        rank
    )
    .execute(&mut tx.tx)
    .await
    .context("Failed to add new full member")?
    .last_insert_rowid();

    link_or_add_discord(tx, Some(mid), discord_id).await?;
    link_or_add_wynn(tx, Some(mid), mcid, ign).await?;

    tx.signal(DBEvent::MemberAdd {
        mid,
        discord_id: Some(discord_id),
        mcid: Some(mcid.to_string()),
        rank,
    });
    Ok(mid)
}

/// Change a member's type to `Full`.
/// Note that this function won't broadcast the `MemberTypeChange` event.
///
/// # Preconditions
/// The member has both discord and mc linked
async fn to_full_member(tx: &mut Transaction, mid: MemberId) -> Result<()> {
    info!(mid, "Updating member type to full");
    query!("UPDATE member SET type=? WHERE oid=?", MemberType::Full, mid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to set member.type to full")?;
    Ok(())
}

/// Change a member's type to `GuildPartial`.
/// Note that this function won't broadcast the `MemberTypeChange` event.
///
/// # Preconditions
/// The member has only mc linked and is in guild
async fn to_guild_partial(tx: &mut Transaction, mid: MemberId) -> Result<()> {
    info!(mid, "Updating member type to guild partial");
    query!("UPDATE member SET type=? WHERE oid=?", MemberType::GuildPartial, mid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to set member.type to guild")?;
    Ok(())
}

/// Add a new discord profile.
///
/// # Preconditions
/// The member exists and is linked to the discord id
async fn add_discord_profile(
    tx: &mut Transaction, mid: Option<MemberId>, discord_id: DiscordId,
) -> Result<()> {
    info!(discord_id, mid, "Creating new discord profile");
    query!("INSERT INTO discord (id,mid) VALUES (?,?)", discord_id, mid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to add discord profile")?;
    tx.signal(DBEvent::DiscordProfileAdd { discord_id, mid });
    Ok(())
}

/// Add a new wynn profile.
///
/// # Preconditions
/// The member exists and is linked to the mcid
async fn add_wynn_profile(tx: &mut Transaction, mid: Option<MemberId>, mcid: &str, ign: &str) -> Result<()> {
    info!(%mcid, mid, "Creating new wynn profile");
    query!("INSERT INTO wynn (id,mid,ign) VALUES (?,?,?)", mcid, mid, ign)
        .execute(&mut tx.tx)
        .await
        .context("Failed to add wynn profile")?;
    tx.signal.signal(DBEvent::WynnProfileAdd { mcid: mcid.to_string(), mid });
    Ok(())
}

/// Add a new guild profile.
///
/// # Preconditions
/// The member exists and is linked to the mcid
/// If a wynn profile with the same mcid exists, its `wynn.guild` is true
async fn add_guild_profile(tx: &mut Transaction, mcid: &str, rank: GuildRank) -> Result<()> {
    info!(%mcid, %rank , "Creating new guild profile");
    query!("INSERT INTO guild (id,rank) VALUES (?,?) ", mcid, rank)
        .execute(&mut tx.tx)
        .await
        .context("Failed to add guild profile")?;
    tx.signal(DBEvent::GuildProfileAdd { mcid: mcid.to_string(), rank });
    Ok(())
}

/// Update a member's rank.
/// Note that this function won't broadcast the `MemberRankChange` event.
pub async fn update_member_rank(tx: &mut Transaction, mid: MemberId, rank: MemberRank) -> Result<()> {
    info!(mid, ?rank, "Updating member rank");
    query!("UPDATE member SET rank=? WHERE oid=?", rank, mid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to update member.rank")?;
    Ok(())
}

/// Reset weekly stats to 0
pub async fn weekly_reset(db: &DB, cache: &Cache) -> Result<()> {
    let v = Vec::new();
    let message_lb = crate::table::stat_leaderboard(cache, db, &Stat::WeeklyMessage, &v).await?;
    let voice_lb = crate::table::stat_leaderboard(cache, db, &Stat::WeeklyVoice, &v).await?;
    let online_lb = crate::table::stat_leaderboard(cache, db, &Stat::WeeklyOnline, &v).await?;
    let xp_lb = crate::table::stat_leaderboard(cache, db, &Stat::WeeklyXp, &v).await?;

    // Transaction isn't used because the following queries aren't related and the WeeklyReset
    // event needs to be broadcasted regardless of error.
    info!("Resetting discord weekly stats");
    let _ = ctx!(
        query!("UPDATE discord SET message_week=0,voice_week=0").execute(&db.pool).await,
        "Failed to set discord weekly stats to 0"
    );
    info!("Resetting wynn weekly stats");
    let _ = ctx!(
        query!("UPDATE wynn SET activity_week=0").execute(&db.pool).await,
        "Failed to set wynn weekly stats to 0"
    );
    info!("Resetting guild weekly stats");
    let _ = ctx!(
        query!("UPDATE guild SET xp_week=0").execute(&db.pool).await,
        "Failed to set guild weekly stats to 0"
    );

    db.signal(DBEvent::WeeklyReset { message_lb, voice_lb, online_lb, xp_lb });
    Ok(())
}

/// Update a discord profile's message count.
pub async fn update_message(tx: &mut Transaction, amount: i64, discord_id: DiscordId) -> Result<()> {
    query!(
        "UPDATE discord SET message=message+?,message_week=message_week+? WHERE id=?",
        amount,
        amount,
        discord_id
    )
    .execute(&mut tx.tx)
    .await
    .context("Failed to update discord.message and discord.message_week")?;
    Ok(())
}

/// Update a discord profile's voice activity.
pub async fn update_voice(tx: &mut Transaction, amount: i64, discord_id: DiscordId) -> Result<()> {
    query!("UPDATE discord SET voice=voice+?,voice_week=voice_week+? WHERE id=?", amount, amount, discord_id)
        .execute(&mut tx.tx)
        .await
        .context("Failed to update discord.voice and discord.voice_week")?;
    Ok(())
}

/// Update a wynn profile's online activity.
pub async fn update_activity(tx: &mut Transaction, mcid: &str, amount: i64) -> Result<()> {
    query!(
        "UPDATE wynn SET activity=activity+?,activity_week=activity_week+? WHERE id=?",
        amount,
        amount,
        mcid
    )
    .execute(&mut tx.tx)
    .await
    .context("Failed to update wynn.activity and wynn.activity_week")?;
    Ok(())
}

/// Update a wynn profile's ign.
pub async fn update_ign(tx: &mut Transaction, mcid: &str, ign: &str) -> Result<()> {
    info!(mcid, ign, "Updating wynn ign");
    query!("UPDATE wynn SET ign=? WHERE id=?", ign, mcid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to update wynn.ign")?;
    Ok(())
}

/// Change a guild profile's guild rank.
pub async fn update_guild_rank(tx: &mut Transaction, mcid: &str, rank: GuildRank) -> Result<()> {
    info!(mcid, %rank, "Updating guild rank");
    query!("UPDATE guild SET rank=? WHERE id=?", rank, mcid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to update guild.rank")?;
    Ok(())
}

/// Update a guild profile's xp tracking.
pub async fn update_xp(tx: &mut Transaction, mcid: &str, amount: i64) -> Result<()> {
    info!(mcid, amount, "Updating guild xp");
    query!("UPDATE guild SET xp=xp+?,xp_week=xp_week+? WHERE id=?", amount, amount, mcid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to update guild.xp and guild.xp_week")?;
    Ok(())
}

/// Update a member's discord link, and return true if the member is removed or demoted to guild
/// partial.
///
/// # Preconditions
/// The new discord profile is unlinked.
#[instrument(skip(tx))]
pub async fn bind_discord(
    tx: &mut Transaction, mid: MemberId, discord_new: Option<DiscordId>,
) -> Result<bool> {
    let discord_old = query!("SELECT discord FROM member where oid=?", mid)
        .fetch_one(&mut tx.tx)
        .await
        .context("Failed to fetch member.discord")?
        .discord;

    // checks for early return
    if discord_old.is_none() && discord_new.is_none() {
        info!("Early return before updating discord binding in member, both old and new are None");
        return Ok(false);
    }
    if let Some(discord_old) = discord_old {
        if let Some(discord_new) = discord_new {
            if discord_old == discord_new {
                info!("Early return before updating discord binding in member, unchanged value");
                return Ok(false);
            }
        }
    }

    info!(discord_old, "Updating discord binding in member");
    query!("UPDATE member SET discord=? WHERE oid=?", discord_new, mid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to set member.discord")?;

    if let Some(discord_old) = discord_old {
        info!("Unlinking old discord profile");
        link_discord(tx, None, discord_old).await?;
    }
    if let Some(discord_new) = discord_new {
        info!("Linking new discord profile");
        link_or_add_discord(tx, Some(mid), discord_new).await?;
    }

    let has_removed = {
        if discord_new.is_none() {
            // Checking if member should be deleted or demoted
            let (_, mcid) = get_member_links(&mut tx.exe(), mid).await?;
            match mcid {
                Some(mcid) => {
                    if is_in_guild(&mut tx.exe(), &mcid).await? {
                        info!("Member is in guild, demote to guild partial");
                        let before = get_member_type(&mut tx.exe(), mid).await?;
                        to_guild_partial(tx, mid).await?;
                        tx.signal(DBEvent::MemberAutoGuildDemote { mid, before });
                    } else {
                        info!("Member not in guild, removing");
                        bind_wynn_unchecked(tx, mid, None).await?;
                        link_wynn(tx, None, &mcid).await?;
                        remove_member_unchecked(tx, mid).await?;
                        tx.signal(DBEvent::WynnProfileUnbind {
                            mid,
                            before: mcid.to_string(),
                            removed: true,
                        });
                        tx.signal(DBEvent::MemberRemove {
                            mid,
                            discord_id: discord_old,
                            mcid: Some(mcid.to_string()),
                        });
                    }
                    true
                }
                None => {
                    info!("Member is empty, removing");
                    remove_member_unchecked(tx, mid).await?;
                    tx.signal(DBEvent::MemberRemove { mid, discord_id: discord_old, mcid: None });
                    true
                }
            }
        } else {
            info!(mid, "Checking if member should be promoted");
            match get_member_type(&mut tx.exe(), mid).await? {
                before @ MemberType::GuildPartial | before @ MemberType::WynnPartial => {
                    to_full_member(tx, mid).await?;
                    tx.signal(DBEvent::MemberFullPromote { mid, before });
                }
                _ => {}
            }
            false
        }
    };

    tx.signal(match discord_new {
        Some(discord_id) => DBEvent::DiscordProfileBind { mid, old: discord_old, new: discord_id },
        None => DBEvent::DiscordProfileUnbind {
            mid,
            before: discord_old.unwrap(),
            removed: has_removed,
        },
    });
    Ok(has_removed)
}

/// Update a member's discord link, and return true if the member is removed or demoted to guild
/// partial.
///
/// # Preconditions
/// The new wynn profile is unlinked.
#[instrument(skip(tx))]
pub async fn bind_wynn(
    tx: &mut Transaction, mid: MemberId, mcid_new: Option<&str>, ign: &str,
) -> Result<bool> {
    let mcid_old = query!("SELECT mcid FROM member where oid=?", mid)
        .fetch_one(&mut tx.tx)
        .await
        .context("Failed to fetch wynn binding from member table")?
        .mcid;

    let member_type = get_member_type(&mut tx.exe(), mid).await?;
    if let MemberType::WynnPartial | MemberType::Full = member_type {
        if let Some(mcid_old) = &mcid_old {
            if mcid_new.is_none() && is_in_guild(&mut tx.exe(), mcid_old).await? {
                // Trying to remove wynn binding on full/wynn partial and player is in guild, so
                // demote to guild partial automatically
                info!(
                    "Removing wynn binding on {}, but player is in guild, so updated to guild partial",
                    member_type
                );

                if let MemberType::Full = member_type {
                    // If member is full, then removing its discord link should also demote it to guild
                    // partial
                    bind_discord(tx, mid, None).await?;
                } else {
                    // If member is wynn partial, then set the member type directly
                    to_guild_partial(tx, mid).await?;
                }
                tx.signal(DBEvent::MemberAutoGuildDemote { mid, before: MemberType::WynnPartial });
                return Ok(true);
            }
        }
    }

    // check for early returns
    if mcid_old.is_none() && mcid_new.is_none() {
        info!(mid, "Early return before updating wynn binding in member, both old and new are None");
        return Ok(false);
    }
    if let Some(mcid_old) = &mcid_old {
        if let Some(mcid_new) = mcid_new {
            if mcid_old.eq(mcid_new) {
                info!(mid, "Early return before updating wynn binding in member, unchanged value");
                return Ok(false);
            }
        }
    }

    info!(?mcid_old, "Updating wynn binding in member");
    bind_wynn_unchecked(tx, mid, mcid_new).await?;

    if let Some(mcid_old) = &mcid_old {
        info!("Unlinking old wynn profile");
        link_wynn(tx, None, &mcid_old).await?;
    }
    if let Some(mcid_new) = mcid_new {
        info!("Linking new wynn profile");
        link_or_add_wynn(tx, Some(mid), &mcid_new, ign).await?;
    }

    let has_removed = if mcid_new.is_none() {
        // Because the case where a wynn partial is in the guild was already taken care of above,
        // the only action here is to remove member.
        info!("No wynn binding, removing member");
        let (discord_id, _) = get_member_links(&mut tx.exe(), mid).await?;
        if discord_id.is_some() {
            // unbind discord then remove member
            bind_discord(tx, mid, None).await?;
        } else {
            // The member is empty, so remove directly
            remove_member_unchecked(tx, mid).await?;
            tx.signal(DBEvent::MemberRemove {
                mid,
                discord_id: None,
                mcid: mcid_old.clone(),
            });
        }
        true
    } else {
        // Checking if member should be promoted
        if let MemberType::DiscordPartial = member_type {
            info!("Added wynn binding to discord partial, promoting to full");
            to_full_member(tx, mid).await?;
            tx.signal(DBEvent::MemberFullPromote { mid, before: MemberType::DiscordPartial });
        }
        false
    };

    tx.signal(match mcid_new {
        Some(mcid) => DBEvent::WynnProfileBind {
            mid,
            old: mcid_old,
            new: mcid.to_string(),
        },
        None => DBEvent::WynnProfileUnbind {
            mid,
            before: mcid_old.unwrap(),
            removed: has_removed,
        },
    });
    Ok(has_removed)
}

/// Set a member's wynn binding to given mcid.
/// Unlike `bind_wynn`, this function doesn't ensure the database integrity,
/// it also doesn't broadcast any events.
///
/// # Preconditions
/// The new wynn profile is unlinked.
async fn bind_wynn_unchecked(tx: &mut Transaction, mid: MemberId, mcid_new: Option<&str>) -> Result<()> {
    info!(mid, mcid_new, "Updating member wynn link");
    query!("UPDATE member SET mcid=? WHERE oid=?", mcid_new, mid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to set wynn binding in member table")?;
    Ok(())
}

/// Update wynn profile's guild status.
/// If wynn or guild profile is missing, new one is created.
/// If a new guild partial is created, their member id is returned.
#[instrument(skip(tx))]
pub async fn bind_wynn_guild(
    tx: &mut Transaction, mcid: &McId, ign: &str, status: bool, rank: GuildRank,
) -> Result<Option<MemberId>> {
    if !wynn_profile_exist(&mut tx.exe(), mcid).await? {
        info!("Adding missing wynn profile");
        add_wynn_profile(tx, None, mcid, ign).await?;
    }
    if !guild_profile_exist(&mut tx.exe(), mcid).await? {
        info!("Adding missing guild profile");
        add_guild_profile(tx, mcid, rank).await?;
    }

    info!("Updating wynn.guild");
    let val = if status { 1 } else { 0 };
    query!("UPDATE wynn SET guild=? WHERE id=?", val, mcid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to update wynn.guild")?;

    match get_wynn_mid(&mut tx.exe(), &mcid).await? {
        Some(mid) => {
            // The only way a member can be affected by wynn.guild update, is if they are a guild
            // partial and wynn.guild is set to false.
            if !status {
                if let MemberType::GuildPartial = get_member_type(&mut tx.exe(), mid).await? {
                    info!("Removing guild partial member because guild profile is unlinked");

                    info!("Unbinding wynn profile");
                    bind_wynn_unchecked(tx, mid, None).await?;
                    link_wynn(tx, None, &mcid).await?;
                    tx.signal(DBEvent::WynnProfileUnbind {
                        mid,
                        before: mcid.to_string(),
                        removed: true,
                    });

                    let (discord, _) = get_member_links(&mut tx.exe(), mid).await?;
                    remove_member_unchecked(tx, mid).await?;
                    tx.signal(DBEvent::MemberRemove {
                        mid,
                        discord_id: discord,
                        mcid: Some(mcid.to_string()),
                    });
                }
            }
        }
        None => {
            // If no member exists and wynn.guild is set to guild, add a corresponding guild
            // partial member.
            if status {
                info!("Adding guild partial member into database");
                let member_rank = rank.to_member_rank();
                let mid = query!(
                    "INSERT INTO member (mcid,type,rank) VALUES (?,?,?)",
                    mcid,
                    MemberType::GuildPartial,
                    member_rank
                )
                .execute(&mut tx.tx)
                .await
                .context("Failed to add guild partial member")?
                .last_insert_rowid();

                link_wynn(tx, Some(mid), mcid).await?;

                tx.signal(DBEvent::WynnProfileBind { mid, old: None, new: mcid.to_string() });
                tx.signal(DBEvent::MemberAdd {
                    mid,
                    discord_id: None,
                    mcid: Some(mcid.to_string()),
                    rank: member_rank,
                });
                return Ok(Some(mid));
            }
        }
    }

    Ok(None)
}

/// Set a discord profile's member binding to given mid.
/// This function doesn't ensure database integrity.
async fn link_discord(tx: &mut Transaction, mid: Option<MemberId>, discord: DiscordId) -> Result<()> {
    info!(mid, discord, "Linking discord profile to member");
    query!("UPDATE discord SET mid=? WHERE id=?", mid, discord)
        .execute(&mut tx.tx)
        .await
        .context("Failed to update discord profile mid link")?;
    Ok(())
}

/// Set a discord profile's member binding to given mid,
/// and if the profile doesn't exists, it is created first.
/// This function doesn't ensure database integrity.
#[instrument(skip(tx))]
async fn link_or_add_discord(tx: &mut Transaction, mid: Option<MemberId>, discord: DiscordId) -> Result<()> {
    if discord_profile_exist(&mut tx.exe(), discord).await? {
        info!("Linking to existing discord profile");
        link_discord(tx, mid, discord).await?;
    } else {
        info!("Linking to newly created discord profile");
        add_discord_profile(tx, mid, discord).await?;
    }
    Ok(())
}

/// Set a wynn profile's member binding to given mid.
/// This function doesn't ensure database integrity.
async fn link_wynn(tx: &mut Transaction, mid: Option<MemberId>, mcid: &str) -> Result<()> {
    info!(mid, %mcid, "Linking wynn profile to member");
    query!("UPDATE wynn SET mid=? WHERE id=?", mid, mcid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to update wynn profile mid link")?;
    Ok(())
}

/// Set a wynn profile's member binding to given mid,
/// and if the profile doesn't exists, it is created first.
/// This function doesn't ensure database integrity.
#[instrument(skip(tx))]
async fn link_or_add_wynn(tx: &mut Transaction, mid: Option<MemberId>, mcid: &str, ign: &str) -> Result<()> {
    if wynn_profile_exist(&mut tx.exe(), mcid).await? {
        info!("Linking to existing wynn profile");
        link_wynn(tx, mid, mcid).await?;
    } else {
        info!("Linking to newly created wynn profile");
        add_wynn_profile(tx, mid, mcid, ign).await?;
    }
    Ok(())
}

/// Delete a member from db.
/// Unlike `remove_member`, this function doesn't ensure database integrity,
/// and also doesn't broadcast `MemberRemove` event.
async fn remove_member_unchecked(tx: &mut Transaction, mid: MemberId) -> Result<()> {
    info!(mid, "Removing member");
    query!("DELETE FROM member WHERE oid=?", mid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to delete from member table")?;
    Ok(())
}

/// Given a member, unbinds all its profiles, and delete it from database
#[instrument(skip(tx))]
pub async fn remove_member(tx: &mut Transaction, mid: MemberId) -> Result<()> {
    let (discord, mcid) = get_member_links(&mut tx.exe(), mid).await?;

    info!(discord, ?mcid, "Removing member with following profile links");

    if mcid.is_some() {
        info!("Unbinding wynn profile");
        if bind_wynn(tx, mid, None, "").await? {
            tx.signal(DBEvent::MemberRemove { mid, discord_id: discord, mcid });
            return Ok(());
        }
    }

    if discord.is_some() {
        info!("Unbinding discord profile");
        if bind_discord(tx, mid, None).await? {
            tx.signal(DBEvent::MemberRemove { mid, discord_id: discord, mcid });
            return Ok(());
        }
    }

    // This should be unreachable unless above functions failed to removed empty member.
    warn!("Deleting invalid state / empty member from table");
    query!("DELETE FROM member WHERE oid=?", mid)
        .execute(&mut tx.tx)
        .await
        .context("Failed to delete from member table")?;
    tx.signal(DBEvent::MemberRemove { mid, discord_id: discord, mcid });
    Ok(())
}
