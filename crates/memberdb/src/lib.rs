pub mod discord;
pub mod error;
pub mod events;
pub mod guild;
pub mod loops;
pub mod member;
pub mod table;
pub mod utils;
pub mod voice_tracker;
pub mod wynn;

use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use serenity::client::Cache;
use serenity::prelude::TypeMapKey;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{query, query_as, Pool, Sqlite, Transaction};
use tokio::sync::broadcast::Receiver;
use tokio::sync::RwLock;
use tracing::{info, instrument, warn};

use crate::discord::*;
use crate::error::*;
use crate::events::{DBEvent, DBSignal};
use crate::guild::*;
use crate::member::*;
use crate::table::Stat;
use crate::wynn::*;

pub struct DBContainer;

impl TypeMapKey for DBContainer {
    type Value = Arc<RwLock<DB>>;
}

impl DBContainer {
    pub async fn new(file: &str, max_conn: u32) -> Arc<RwLock<DB>> {
        let db = DB::new(file, max_conn).await;
        Arc::new(RwLock::new(db))
    }
}

#[derive(Debug)]
pub struct DB {
    pub pool: Pool<Sqlite>,
    pub signal: DBSignal,
}

impl DB {
    pub async fn new(file: &str, max_conn: u32) -> Self {
        Self {
            pool: connect_db(file, max_conn).await,
            signal: DBSignal::new(64),
        }
    }

    pub async fn begin(&self) -> Result<Transaction<'static, Sqlite>> {
        self.pool.begin().await.context("Failed to begin db transaction")
    }

    pub fn connect(&self) -> Receiver<Arc<DBEvent>> {
        self.signal.connect()
    }

    pub fn signal(&self, event: DBEvent) {
        self.signal.signal(event);
    }
}

pub async fn connect_db(file: &str, max_conn: u32) -> Pool<Sqlite> {
    let db = SqlitePoolOptions::new()
        .max_connections(max_conn)
        .connect_with(SqliteConnectOptions::new().filename(file).create_if_missing(true))
        .await
        .expect("Couldn't connect to database");
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("Couldn't run database migrations");
    db
}

/// Add discord partial member, if profile doesn't exist, it is created.
///
/// # Errors
/// `DBError::MemberAlreadyExist` if the given discord id is already binded to another member.
#[instrument(skip(db))]
pub async fn add_member_discord(db: &DB, discord_id: DiscordId, rank: MemberRank) -> Result<MemberId> {
    if let Some(mid) = get_discord_mid(&db, discord_id).await? {
        return Err(DBError::MemberAlreadyExist(mid).into());
    }

    info!("Adding discord partial member into database");
    let trans = db.begin().await?;
    let mid = query!(
        "INSERT INTO member (discord,type,rank) VALUES (?,?,?)",
        discord_id,
        MemberType::DiscordPartial,
        rank
    )
    .execute(&db.pool)
    .await
    .context("Failed to add discord partial member to database")?
    .last_insert_rowid();

    link_or_add_discord(&db, Some(mid), discord_id).await?;

    trans.commit().await.context("Failed to commit db transaction")?;
    db.signal(DBEvent::MemberAdd {
        mid,
        discord_id: Some(discord_id),
        mcid: None,
        rank,
    });
    Ok(mid)
}

/// Add wynn partial member, if profile doesn't exist, it is created.
///
/// # Errors
/// `DBErr::MemberAlreadyExist` if the given mc id is already binded to another member.
#[instrument(skip(db))]
pub async fn add_member_wynn(db: &DB, mcid: &str, rank: MemberRank, ign: &str) -> Result<MemberId> {
    if let Some(mid) = get_wynn_mid(&db, mcid).await? {
        return Err(DBError::MemberAlreadyExist(mid).into());
    }

    info!("Adding wynn partial member into database");
    let trans = db.begin().await?;
    let mid =
        query!("INSERT INTO member (mcid,type,rank) VALUES (?,?,?)", mcid, MemberType::WynnPartial, rank)
            .execute(&db.pool)
            .await
            .context("Failed to add wynn partial member")?
            .last_insert_rowid();

    link_or_add_wynn(&db, Some(mid), mcid, ign).await?;

    trans.commit().await?;
    db.signal(DBEvent::MemberAdd {
        mid,
        discord_id: None,
        mcid: Some(mcid.to_string()),
        rank,
    });
    Ok(mid)
}

/// Add full member, if any profiles doesn't exist, it is created.
///
/// # Errors
/// `DBError::LinkOverride` if the given mc id or discord id is already binded to another member.
#[instrument(skip(db))]
pub async fn add_member(
    db: &DB, discord_id: DiscordId, mcid: &str, ign: &str, rank: MemberRank,
) -> Result<MemberId> {
    if let Some(mid) = get_discord_mid(&db, discord_id).await? {
        return Err(DBError::LinkOverride(ProfileType::Discord, mid).into());
    }
    if let Some(mid) = get_wynn_mid(&db, mcid).await? {
        return Err(DBError::LinkOverride(ProfileType::Wynn, mid).into());
    }

    info!("Adding full member into database");
    let trans = db.begin().await?;
    let mid = query!(
        "INSERT INTO member (discord,mcid,type,rank) VALUES (?,?,?,?)",
        discord_id,
        mcid,
        MemberType::Full,
        rank
    )
    .execute(&db.pool)
    .await
    .context("Failed to add new full member")?
    .last_insert_rowid();

    link_or_add_discord(&db, Some(mid), discord_id).await?;
    link_or_add_wynn(&db, Some(mid), mcid, ign).await?;

    trans.commit().await?;
    db.signal(DBEvent::MemberAdd {
        mid,
        discord_id: Some(discord_id),
        mcid: Some(mcid.to_string()),
        rank,
    });
    Ok(mid)
}

/// Change a member's type to `Full`.
/// Note that this function won't broadcast the `MemberTypeChange` event.
async fn promote(db: &DB, mid: MemberId) -> Result<()> {
    info!(mid, "Updating member type to full");
    query!("UPDATE member SET type=? WHERE oid=?", MemberType::Full, mid)
        .execute(&db.pool)
        .await
        .context("Failed to set member.type to full")?;
    Ok(())
}

/// Change a member's type to `GuildPartial`.
/// Note that this function won't broadcast the `MemberTypeChange` event.
async fn demote_to_guild(db: &DB, mid: MemberId) -> Result<()> {
    info!(mid, "Updating member type to guild partial");
    query!("UPDATE member SET type=? WHERE oid=?", MemberType::GuildPartial, mid)
        .execute(&db.pool)
        .await
        .context("Failed to set member.type to guild")?;
    Ok(())
}

/// Add a new discord profile.
async fn add_discord_profile(db: &DB, mid: Option<MemberId>, discord_id: DiscordId) -> Result<()> {
    info!(discord_id, mid, "Creating new discord profile");
    query!("INSERT INTO discord (id,mid) VALUES (?,?)", discord_id, mid)
        .execute(&db.pool)
        .await
        .context("Failed to add discord profile")?;
    db.signal(DBEvent::DiscordProfileAdd { discord_id, mid });
    Ok(())
}

/// Add a new wynn profile.
async fn add_wynn_profile(db: &DB, mid: Option<MemberId>, mcid: &str, ign: &str) -> Result<()> {
    info!(%mcid, mid, "Creating new wynn profile");
    query!("INSERT INTO wynn (id,mid,ign) VALUES (?,?,?)", mcid, mid, ign)
        .execute(&db.pool)
        .await
        .context("Failed to add wynn profile")?;
    db.signal(DBEvent::WynnProfileAdd { mcid: mcid.to_string(), mid });
    Ok(())
}

/// Add a new guild profile.
async fn add_guild_profile(db: &DB, mcid: &str, rank: GuildRank) -> Result<()> {
    info!(%mcid, %rank , "Creating new guild profile");
    query!("INSERT INTO guild (id,rank) VALUES (?,?) ", mcid, rank)
        .execute(&db.pool)
        .await
        .context("Failed to add guild profile")?;
    db.signal(DBEvent::GuildProfileAdd { mcid: mcid.to_string(), rank });
    Ok(())
}

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

/// Get a member's type
pub async fn get_member_type(db: &DB, mid: MemberId) -> Result<MemberType> {
    let row = query!("SELECT type AS member_type FROM member WHERE oid=?", mid)
        .fetch_one(&db.pool)
        .await
        .context("Failed to fetch member.type")?;
    Ok(MemberType::from_str(&row.member_type)?)
}

/// Update a member's rank.
/// Note that this function won't broadcast the `MemberRankChange` event.
pub async fn update_member_rank(db: &DB, mid: MemberId, rank: MemberRank) -> Result<()> {
    info!(mid, ?rank, "Updating member rank");
    query!("UPDATE member SET rank=? WHERE oid=?", rank, mid)
        .execute(&db.pool)
        .await
        .context("Failed to update member.rank")?;
    Ok(())
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

async fn weekly_reset(db: &DB, cache: &Cache) -> Result<()> {
    let message_lb = crate::table::stat_leaderboard(cache, db, &Stat::Message(true), &None, true).await?;
    let voice_lb = crate::table::stat_leaderboard(cache, db, &Stat::Voice(true), &None, true).await?;
    let online_lb = crate::table::stat_leaderboard(cache, db, &Stat::Online(true), &None, true).await?;
    let xp_lb = crate::table::stat_leaderboard(cache, db, &Stat::Xp(true), &None, true).await?;

    info!("Resetting discord weekly stats");
    query!("UPDATE discord SET message_week=0,voice_week=0")
        .execute(&db.pool)
        .await
        .context("Failed to set discord weekly stats to 0")?;
    info!("Resetting wynn weekly stats");
    query!("UPDATE wynn SET activity_week=0")
        .execute(&db.pool)
        .await
        .context("Failed to set wynn weekly stats to 0")?;
    info!("Resetting guild weekly stats");
    query!("UPDATE guild SET xp_week=0")
        .execute(&db.pool)
        .await
        .context("Failed to set guild weekly stats to 0")?;

    db.signal(DBEvent::WeeklyReset { message_lb, voice_lb, online_lb, xp_lb });
    Ok(())
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

/// Update a discord profile's message count (assuming the profile exists).
pub async fn update_message(db: &DB, amount: i64, discord_id: DiscordId) -> Result<()> {
    query!(
        "UPDATE discord SET message=message+?,message_week=message_week+? WHERE id=?",
        amount,
        amount,
        discord_id
    )
    .execute(&db.pool)
    .await
    .context("Failed to update discord.message and discord.message_week")?;
    Ok(())
}

/// Update a discord profile's voice activity (assuming the profile exists).
pub async fn update_voice(db: &DB, amount: i64, discord_id: DiscordId) -> Result<()> {
    info!(discord_id, amount, "Updating discord voice time");
    query!("UPDATE discord SET voice=voice+?,voice_week=voice_week+? WHERE id=?", amount, amount, discord_id)
        .execute(&db.pool)
        .await
        .context("Failed to update discord.voice and discord.voice_week")?;
    Ok(())
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

/// Update a wynn profile's online activity (assuming the profile exists).
pub async fn update_activity(db: &DB, mcid: &str, amount: i64) -> Result<()> {
    query!(
        "UPDATE wynn SET activity=activity+?,activity_week=activity_week+? WHERE id=?",
        amount,
        amount,
        mcid
    )
    .execute(&db.pool)
    .await
    .context("Failed to update wynn.activity and wynn.activity_week")?;
    Ok(())
}

/// Update a wynn profile's ign (assuming the profile exists).
pub async fn update_ign(db: &DB, mcid: &str, ign: &str) -> Result<()> {
    info!(mcid, ign, "Updating wynn ign");
    query!("UPDATE wynn SET ign=? WHERE id=?", ign, mcid)
        .execute(&db.pool)
        .await
        .context("Failed to update wynn.ign")?;
    Ok(())
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

/// Get a guild profile's guild rank (assuming the profile exists).
pub async fn get_guild_rank(db: &DB, mcid: &str) -> Result<GuildRank> {
    let rank = query!("SELECT rank FROM guild WHERE id=?", mcid)
        .fetch_one(&db.pool)
        .await
        .context("Failed to get guild.rank")?
        .rank;
    Ok(GuildRank::from_str(&rank)?)
}

/// Change a guild profile's guild rank (assuming the profile exists).
pub async fn update_guild_rank(db: &DB, mcid: &str, rank: GuildRank) -> Result<()> {
    info!(mcid, %rank, "Updating guild rank");
    query!("UPDATE guild SET rank=? WHERE id=?", rank, mcid)
        .execute(&db.pool)
        .await
        .context("Failed to update guild.rank")?;
    Ok(())
}

/// Get a guild profile's total xp (assuming the profile exists).
pub async fn get_xp(db: &DB, mcid: &str) -> Result<i64> {
    Ok(query!("SELECT xp FROM guild WHERE id=?", mcid)
        .fetch_one(&db.pool)
        .await
        .context("Failed to get guild.xp")?
        .xp)
}

/// Update a guild profile's xp tracking (assuming the profile exists).
pub async fn update_xp(db: &DB, mcid: &str, amount: i64) -> Result<()> {
    info!(mcid, amount, "Updating guild xp");
    query!("UPDATE guild SET xp=xp+?,xp_week=xp_week+? WHERE id=?", amount, amount, mcid)
        .execute(&db.pool)
        .await
        .context("Failed to update guild.xp and guild.xp_week")?;
    Ok(())
}

/// Updates member's discord binding, unlinking the old profile (if there is any),
/// and linking and/or create the new profile (if specified).
///
/// This function doesn't broadcast `MemberRemove` event.
///
/// If this is called to unbind discord profile, it is then checked to see if the member is empty afterward,
/// and return `Ok(true)` if it is deleted because of that.
/// If this is called to bind discord profile, and the member if a wynn / guild partial member,
/// they are also promoted to full member
///
/// # Errors
/// `DBError::LinkOverride` if the given discord id is already binded to another member.
#[instrument(skip(db))]
pub async fn bind_discord(db: &DB, mid: MemberId, discord_new: Option<DiscordId>) -> Result<bool> {
    if let Some(discord_new) = discord_new {
        if let Some(mid) = get_discord_mid(&db, discord_new).await? {
            return Err(DBError::LinkOverride(ProfileType::Discord, mid).into());
        }
    }

    let discord_old = query!("SELECT discord FROM member where oid=?", mid)
        .fetch_one(&db.pool)
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
    let trans = db.begin().await?;
    query!("UPDATE member SET discord=? WHERE oid=?", discord_new, mid)
        .execute(&db.pool)
        .await
        .context("Failed to set member.discord")?;

    if let Some(discord_old) = discord_old {
        info!("Unlinking old discord profile");
        link_discord(&db, None, discord_old).await?;
    }
    if let Some(discord_new) = discord_new {
        info!("Linking new discord profile");
        link_or_add_discord(&db, Some(mid), discord_new).await?;
    }

    let has_removed = {
        if discord_new.is_none() {
            // Checking if member should be deleted or demoted
            let (_, mcid) = get_member_links(&db, mid).await?;
            match mcid {
                Some(mcid) => {
                    if is_in_guild(&db, &mcid).await? {
                        info!("Member is in guild, demote to guild partial");
                        let before = get_member_type(&db, mid).await?;
                        demote_to_guild(&db, mid).await?;
                        db.signal(DBEvent::MemberAutoGuildDemote { mid, before });
                        false
                    } else {
                        info!("Member not in guild, removing");
                        bind_wynn_unchecked(&db, mid, None).await?;
                        link_wynn(&db, None, &mcid).await?;
                        remove_member_unchecked(&db, mid).await?;
                        db.signal(DBEvent::WynnProfileUnbind {
                            mid,
                            before: mcid.to_string(),
                            removed: true,
                        });
                        true
                    }
                }
                None => {
                    info!("Member is empty, removing");
                    remove_member_unchecked(&db, mid).await?;
                    true
                }
            }
        } else {
            info!(mid, "Checking if member should be promoted");
            match get_member_type(&db, mid).await? {
                before @ MemberType::GuildPartial | before @ MemberType::WynnPartial => {
                    promote(&db, mid).await?;
                    db.signal(DBEvent::MemberFullPromote { mid, before });
                }
                _ => {}
            }
            false
        }
    };

    trans.commit().await?;
    db.signal(match discord_new {
        Some(discord_id) => DBEvent::DiscordProfileBind { mid, old: discord_old, new: discord_id },
        None => DBEvent::DiscordProfileUnbind {
            mid,
            before: discord_old.unwrap(),
            removed: has_removed,
        },
    });
    Ok(has_removed)
}

/// Updates member's wynn binding, unlinking the old profile (if there is any),
/// and linking and/or create the new profile (if specified).
///
/// This function doesn't broadcast `MemberRemove` event.
///
/// If this is called to unbind wynn profile, and the player is in guild, then it is turned to guild partial,
/// otheriwse it is delted and and return `Ok(true)` because of that.
/// If this is called to bind wynn profile, and the member is a discord partial member,
/// they are also promoted to full member.
///
/// # Errors
/// `DBError::linkOverride` if the given mc id is already binded to another member.
/// `DBError::WrongMemberType` if the given member is a guild partial.
#[instrument(skip(db))]
pub async fn bind_wynn(db: &DB, mid: MemberId, mcid_new: Option<&str>, ign: &str) -> Result<bool> {
    if let Some(mcid_new) = mcid_new {
        if let Some(mid) = get_wynn_mid(&db, mcid_new).await? {
            return Err(DBError::LinkOverride(ProfileType::Wynn, mid).into());
        }
    }

    let mcid_old = query!("SELECT mcid FROM member where oid=?", mid)
        .fetch_one(&db.pool)
        .await
        .context("Failed to fetch wynn binding from member table")?
        .mcid;

    match get_member_type(&db, mid).await? {
        MemberType::GuildPartial => return Err(DBError::WrongMemberType(MemberType::GuildPartial).into()),
        MemberType::WynnPartial => {
            if let Some(mcid_old) = &mcid_old {
                if mcid_new.is_none() && is_in_guild(&db, mcid_old).await? {
                    info!("Removing wynn binding on wynn partial, but player is in guild, so updated to guild partial");
                    demote_to_guild(&db, mid).await?;
                    db.signal(DBEvent::MemberAutoGuildDemote { mid, before: MemberType::WynnPartial });
                    return Ok(false);
                }
            }
        }
        _ => {}
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
    let trans = db.begin().await?;
    bind_wynn_unchecked(&db, mid, mcid_new).await?;

    if let Some(mcid_old) = &mcid_old {
        info!("Unlinking old wynn profile");
        link_wynn(&db, None, &mcid_old).await?;
    }
    if let Some(mcid_new) = mcid_new {
        info!("Linking new wynn profile");
        link_or_add_wynn(&db, Some(mid), &mcid_new, ign).await?;
    }

    let has_removed = if mcid_new.is_none() {
        // Because the member can only be full or wynn partial at this point,
        // and the case where a wynn partial is in the guild was already taken care of above,
        // then the member has to be removed.
        info!("No wynn binding, removing member");
        let (discord_id, _) = get_member_links(&db, mid).await?;
        if discord_id.is_some() {
            // unbind discord then remove member
            bind_discord(&db, mid, None).await?;
        } else {
            // The member is empty, so remove directly
            remove_member_unchecked(&db, mid).await?;
        }
        true
    } else {
        // Checking if member should be promoted
        match get_member_type(&db, mid).await? {
            MemberType::DiscordPartial => {
                info!("Added wynn binding to discord partial, promoting to full");
                promote(&db, mid).await?;
                db.signal(DBEvent::MemberFullPromote { mid, before: MemberType::DiscordPartial });
            }
            _ => {}
        }
        false
    };

    trans.commit().await?;
    db.signal(match mcid_new {
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

/// Set a member's wynn binding to given mcid (assuming the member exists).
/// Unlike `bind_wynn`, this function doesn't ensure the resulting db structure is valid,
/// it also doesn't broadcast the `WynnProfileBind` or `WynnProfileUnbind` event.
async fn bind_wynn_unchecked(db: &DB, mid: MemberId, mcid_new: Option<&str>) -> Result<()> {
    info!(mid, mcid_new, "Updating member wynn link");
    query!("UPDATE member SET mcid=? WHERE oid=?", mcid_new, mid)
        .execute(&db.pool)
        .await
        .context("Failed to set wynn binding in member table")?;
    Ok(())
}

/// Updates binding between guild and wynn profile (if any of them doesn't exist, new one is created).
/// If it is used to unbind guild, and the linked member is a guild partial member,
/// the member is removed.
/// If it is used to bind guild and wynn profile isn't linked to a member,
/// a new guild partial member is created.
#[instrument(skip(db))]
pub async fn bind_wynn_guild(
    db: &DB, mcid: &McId, ign: &str, status: bool, rank: GuildRank,
) -> Result<Option<MemberId>> {
    if !wynn_profile_exist(&db, mcid).await? {
        info!("Adding missing wynn profile");
        add_wynn_profile(&db, None, mcid, ign).await?;
    }
    if !guild_profile_exist(&db, mcid).await? {
        info!("Adding missing guild profile");
        add_guild_profile(&db, mcid, rank).await?;
    }

    info!("Updating wynn.guild");
    let trans = db.begin().await?;
    let val = if status { 1 } else { 0 };
    query!("UPDATE wynn SET guild=? WHERE id=?", val, mcid)
        .execute(&db.pool)
        .await
        .context("Failed to update wynn.guild")?;

    match get_wynn_mid(&db, &mcid).await? {
        Some(mid) => {
            if !status {
                if let MemberType::GuildPartial = get_member_type(&db, mid).await? {
                    info!("Removing guild partial member because guild profile is unlinked");

                    info!("Unbinding wynn profile");
                    bind_wynn_unchecked(&db, mid, None).await?;
                    link_wynn(&db, None, &mcid).await?;
                    db.signal(DBEvent::WynnProfileUnbind {
                        mid,
                        before: mcid.to_string(),
                        removed: true,
                    });

                    let (discord, _) = get_member_links(&db, mid).await?;
                    remove_member_unchecked(&db, mid).await?;
                    db.signal(DBEvent::MemberRemove {
                        mid,
                        discord_id: discord,
                        mcid: Some(mcid.to_string()),
                    });
                }
            }
        }
        None => {
            if status {
                info!("Adding guild partial member into database");
                let member_rank = rank.to_member_rank();
                let mid = query!(
                    "INSERT INTO member (mcid,type,rank) VALUES (?,?,?)",
                    mcid,
                    MemberType::GuildPartial,
                    member_rank
                )
                .execute(&db.pool)
                .await
                .context("Failed to add guild partial member")?
                .last_insert_rowid();
                link_wynn(&db, Some(mid), mcid).await?;
                trans.commit().await?;

                db.signal(DBEvent::WynnProfileBind { mid, old: None, new: mcid.to_string() });
                db.signal(DBEvent::MemberAdd {
                    mid,
                    discord_id: None,
                    mcid: Some(mcid.to_string()),
                    rank: member_rank,
                });
                return Ok(Some(mid));
            }
        }
    }

    trans.commit().await?;
    Ok(None)
}

/// Set a discord profile's member binding to given mid (assuming the profile exists).
async fn link_discord(db: &DB, mid: Option<MemberId>, discord: DiscordId) -> Result<()> {
    info!(mid, discord, "Linking discord profile to member");
    query!("UPDATE discord SET mid=? WHERE id=?", mid, discord)
        .execute(&db.pool)
        .await
        .context("Failed to update discord profile mid link")?;
    Ok(())
}

/// Set a discord profile's member binding to given mid,
/// and if the profile doesn't exists, it is created first.
#[instrument(skip(db))]
async fn link_or_add_discord(db: &DB, mid: Option<MemberId>, discord: DiscordId) -> Result<()> {
    if discord_profile_exist(&db, discord).await? {
        info!("Linking to existing discord profile");
        link_discord(&db, mid, discord).await?;
    } else {
        info!("Linking to newly created discord profile");
        add_discord_profile(&db, mid, discord).await?;
    }
    Ok(())
}

/// Set a wynn profile's member binding to given mid (assuming the profile exists).
async fn link_wynn(db: &DB, mid: Option<MemberId>, mcid: &str) -> Result<()> {
    info!(mid, %mcid, "Linking wynn profile to member");
    query!("UPDATE wynn SET mid=? WHERE id=?", mid, mcid)
        .execute(&db.pool)
        .await
        .context("Failed to update wynn profile mid link")?;
    Ok(())
}

/// Set a wynn profile's member binding to given mid,
/// and if the profile doesn't exists, it is created first.
#[instrument(skip(db))]
async fn link_or_add_wynn(db: &DB, mid: Option<MemberId>, mcid: &str, ign: &str) -> Result<()> {
    if wynn_profile_exist(&db, mcid).await? {
        info!("Linking to existing wynn profile");
        link_wynn(&db, mid, mcid).await?;
    } else {
        info!("Linking to newly created wynn profile");
        add_wynn_profile(&db, mid, mcid, ign).await?;
    }
    Ok(())
}

/// Delete a member from db.
/// Unlike `remove_member`, this function doesn't ensure the profile bindings are severed,
/// and also doesn't broadcast `MemberRemove` event.
async fn remove_member_unchecked(db: &DB, mid: MemberId) -> Result<()> {
    info!(mid, "Removing member");
    query!("DELETE FROM member WHERE oid=?", mid)
        .execute(&db.pool)
        .await
        .context("Failed to delete from member table")?;
    Ok(())
}

/// Given a member, unbinds all its profiles, and delete it from database
///
/// # Errors
/// `DBError::WrongMemberType` if the given member is a guild partial.
#[instrument(skip(db))]
pub async fn remove_member(db: &DB, mid: MemberId) -> Result<()> {
    let (discord, mcid) = get_member_links(&db, mid).await?;

    info!(discord, ?mcid, "Removing member with following profile links");
    let trans = db.begin().await?;

    // wynn profile is removed first so it can't be demoted to guild partial
    if mcid.is_some() {
        info!("Unbinding wynn profile");
        if bind_wynn(&db, mid, None, "").await? {
            trans.commit().await?;
            db.signal(DBEvent::MemberRemove { mid, discord_id: discord, mcid });
            return Ok(());
        }
    }

    if discord.is_some() {
        info!("Unbinding discord profile");
        if bind_discord(&db, mid, None).await? {
            trans.commit().await?;
            db.signal(DBEvent::MemberRemove { mid, discord_id: discord, mcid });
            return Ok(());
        }
    }

    warn!("Deleting invalid state / empty member from table");
    query!("DELETE FROM member WHERE oid=?", mid)
        .execute(&db.pool)
        .await
        .context("Failed to delete from member table")?;
    trans.commit().await?;
    db.signal(DBEvent::MemberRemove { mid, discord_id: discord, mcid });
    Ok(())
}
