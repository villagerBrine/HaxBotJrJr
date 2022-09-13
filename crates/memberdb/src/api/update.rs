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

use crate::events::DBEvent;
use crate::model::discord::DiscordId;
use crate::model::guild::GuildRank;
use crate::model::member::{MemberId, MemberRank, MemberType};
use crate::model::wynn::McId;
use crate::model::db::Stat;
use crate::{Transaction, DB};

impl MemberId {
    /// Add discord partial member, if profile doesn't exist, it is created.
    ///
    /// # Preconditions
    /// The given discord id is unlinked
    #[instrument(skip(tx))]
    pub async fn add_discord_partial(
        tx: &mut Transaction, discord_id: DiscordId, rank: MemberRank,
    ) -> Result<Self> {
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
        let mid = Self(mid);

        discord_id.link_or_create_unchecked(tx, Some(mid)).await?;

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
    pub async fn add_wynn_partial(
        tx: &mut Transaction, mcid: &McId, rank: MemberRank, ign: &str,
    ) -> Result<Self> {
        info!("Adding wynn partial member into database");
        let mid =
            query!("INSERT INTO member (mcid,type,rank) VALUES (?,?,?)", mcid, MemberType::WynnPartial, rank)
                .execute(&mut tx.tx)
                .await
                .context("Failed to add wynn partial member")?
                .last_insert_rowid();
        let mid = Self(mid);

        mcid.link_or_create_unchecked(tx, Some(mid), ign).await?;

        tx.signal(DBEvent::MemberAdd {
            mid,
            discord_id: None,
            mcid: Some(mcid.clone()),
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
        tx: &mut Transaction, discord_id: DiscordId, mcid: &McId, ign: &str, rank: MemberRank,
    ) -> Result<Self> {
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
        let mid = Self(mid);

        discord_id.link_or_create_unchecked(tx, Some(mid)).await?;
        mcid.link_or_create_unchecked(tx, Some(mid), ign).await?;

        tx.signal(DBEvent::MemberAdd {
            mid,
            discord_id: Some(discord_id),
            mcid: Some(mcid.clone()),
            rank,
        });
        Ok(mid)
    }

    /// Change a member's type to `Full`.
    /// Note that this function won't broadcast the `MemberTypeChange` event.
    ///
    /// # Preconditions
    /// The member has both discord and mc linked
    async fn to_full_member(&self, tx: &mut Transaction) -> Result<()> {
        info!(?self, "Updating member type to full");
        query!("UPDATE member SET type=? WHERE oid=?", MemberType::Full, self)
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
    async fn to_guild_partial(&self, tx: &mut Transaction) -> Result<()> {
        info!(?self, "Updating member type to guild partial");
        query!("UPDATE member SET type=? WHERE oid=?", MemberType::GuildPartial, self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to set member.type to guild")?;
        Ok(())
    }

    /// Update a member's rank.
    /// Note that this function won't broadcast the `MemberRankChange` event.
    pub async fn set_rank(self, tx: &mut Transaction, rank: MemberRank) -> Result<()> {
        info!(?self, ?rank, "Updating member rank");
        query!("UPDATE member SET rank=? WHERE oid=?", rank, self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to update member.rank")?;
        Ok(())
    }

    /// Update member's discord link, and return true if the member is removed or demoted to guild
    /// partial.
    ///
    /// # Preconditions
    /// The new discord profile is unlinked.
    #[instrument(skip(tx))]
    pub async fn bind_discord(&self, tx: &mut Transaction, discord_new: Option<DiscordId>) -> Result<bool> {
        let discord_old = query!("SELECT discord FROM member where oid=?", self)
            .fetch_one(&mut tx.tx)
            .await
            .context("Failed to fetch member.discord")?
            .discord
            .map(|id| DiscordId(id));

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

        info!(?discord_old, "Updating discord binding in member");
        query!("UPDATE member SET discord=? WHERE oid=?", discord_new, self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to set member.discord")?;

        if let Some(discord_old) = discord_old {
            info!("Unlinking old discord profile");
            discord_old.link_unchecked(tx, None).await?;
        }
        if let Some(discord_new) = discord_new {
            info!("Linking new discord profile");
            discord_new.link_or_create_unchecked(tx, Some(*self)).await?;
        }

        let has_removed = {
            if discord_new.is_none() {
                // Checking if member should be deleted or demoted
                let (_, mcid) = self.links(&mut tx.exe()).await?;
                match mcid {
                    Some(mcid) => {
                        if mcid.in_guild(&mut tx.exe()).await? {
                            info!("Member is in guild, demote to guild partial");
                            let before = self.kind(&mut tx.exe()).await?;
                            self.to_guild_partial(tx).await?;
                            tx.signal(DBEvent::MemberAutoGuildDemote { mid: *self, before });
                        } else {
                            info!("Member not in guild, removing");
                            self.bind_wynn_unchecked(tx, None).await?;
                            mcid.link_unchecked(tx, None).await?;
                            self.remove_unchecked(tx).await?;
                            tx.signal(DBEvent::WynnProfileUnbind {
                                mid: *self,
                                before: mcid.clone(),
                                removed: true,
                            });
                            tx.signal(DBEvent::MemberRemove {
                                mid: *self,
                                discord_id: discord_old,
                                mcid: Some(mcid.clone()),
                            });
                        }
                        true
                    }
                    None => {
                        info!("Member is empty, removing");
                        self.remove_unchecked(tx).await?;
                        tx.signal(DBEvent::MemberRemove {
                            mid: *self,
                            discord_id: discord_old,
                            mcid: None,
                        });
                        true
                    }
                }
            } else {
                info!(?self, "Checking if member should be promoted");
                match self.kind(&mut tx.exe()).await? {
                    before @ MemberType::GuildPartial | before @ MemberType::WynnPartial => {
                        self.to_full_member(tx).await?;
                        tx.signal(DBEvent::MemberFullPromote { mid: *self, before });
                    }
                    _ => {}
                }
                false
            }
        };

        tx.signal(match discord_new {
            Some(discord_id) => DBEvent::DiscordProfileBind {
                mid: *self,
                old: discord_old,
                new: discord_id,
            },
            None => DBEvent::DiscordProfileUnbind {
                mid: *self,
                before: discord_old.unwrap(),
                removed: has_removed,
            },
        });
        Ok(has_removed)
    }

    /// Update member's discord link, and return true if the member is removed or demoted to guild
    /// partial.
    ///
    /// # Preconditions
    /// The new wynn profile is unlinked.
    #[instrument(skip(tx))]
    pub async fn bind_wynn(&self, tx: &mut Transaction, mcid_new: Option<&McId>, ign: &str) -> Result<bool> {
        let mcid_old = query!("SELECT mcid FROM member where oid=?", self)
            .fetch_one(&mut tx.tx)
            .await
            .context("Failed to fetch wynn binding from member table")?
            .mcid;
        let mcid_old = mcid_old.map(|id| McId(id));

        let member_type = self.kind(&mut tx.exe()).await?;
        if let MemberType::WynnPartial | MemberType::Full = member_type {
            if let Some(mcid_old) = &mcid_old {
                if mcid_new.is_none() && mcid_old.in_guild(&mut tx.exe()).await? {
                    // Trying to remove wynn binding on full/wynn partial and player is in guild, so
                    // demote to guild partial automatically
                    info!(
                        "Removing wynn binding on {}, but player is in guild, so updated to guild partial",
                        member_type
                    );

                    if let MemberType::Full = member_type {
                        // If member is full, then removing its discord link should also demote it to guild
                        // partial
                        self.bind_discord(tx, None).await?;
                    } else {
                        // If member is wynn partial, then set the member type directly
                        self.to_guild_partial(tx).await?;
                    }
                    tx.signal(DBEvent::MemberAutoGuildDemote {
                        mid: *self,
                        before: MemberType::WynnPartial,
                    });
                    return Ok(true);
                }
            }
        }

        // check for early returns
        if mcid_old.is_none() && mcid_new.is_none() {
            info!(?self, "Early return before updating wynn binding in member, both old and new are None");
            return Ok(false);
        }
        if let Some(mcid_old) = &mcid_old {
            if let Some(mcid_new) = mcid_new {
                if mcid_old.eq(mcid_new) {
                    info!(?self, "Early return before updating wynn binding in member, unchanged value");
                    return Ok(false);
                }
            }
        }

        info!(?mcid_old, "Updating wynn binding in member");
        self.bind_wynn_unchecked(tx, mcid_new).await?;

        if let Some(mcid_old) = &mcid_old {
            info!("Unlinking old wynn profile");
            mcid_old.link_unchecked(tx, None).await?;
        }
        if let Some(mcid_new) = mcid_new {
            info!("Linking new wynn profile");
            mcid_new.link_or_create_unchecked(tx, Some(*self), ign).await?;
        }

        let has_removed = if mcid_new.is_none() {
            // Because the case where a wynn partial is in the guild was already taken care of above,
            // the only action here is to remove member.
            info!("No wynn binding, removing member");
            let (discord_id, _) = self.links(&mut tx.exe()).await?;
            if discord_id.is_some() {
                // unbind discord then remove member
                self.bind_discord(tx, None).await?;
            } else {
                // The member is empty, so remove directly
                self.remove_unchecked(tx).await?;
                tx.signal(DBEvent::MemberRemove {
                    mid: *self,
                    discord_id: None,
                    mcid: mcid_old.clone(),
                });
            }
            true
        } else {
            // Checking if member should be promoted
            if let MemberType::DiscordPartial = member_type {
                info!("Added wynn binding to discord partial, promoting to full");
                self.to_full_member(tx).await?;
                tx.signal(DBEvent::MemberFullPromote {
                    mid: *self,
                    before: MemberType::DiscordPartial,
                });
            }
            false
        };

        tx.signal(match mcid_new {
            Some(mcid) => DBEvent::WynnProfileBind {
                mid: *self,
                old: mcid_old,
                new: mcid.clone(),
            },
            None => DBEvent::WynnProfileUnbind {
                mid: *self,
                before: mcid_old.unwrap(),
                removed: has_removed,
            },
        });
        Ok(has_removed)
    }

    /// Set member's wynn binding to given mcid.
    /// Unlike `bind_wynn`, this function doesn't ensure the database integrity,
    /// it also doesn't broadcast any events.
    ///
    /// # Preconditions
    /// The new wynn profile is unlinked.
    async fn bind_wynn_unchecked(&self, tx: &mut Transaction, mcid_new: Option<&McId>) -> Result<()> {
        info!(?self, ?mcid_new, "Updating member wynn link");
        query!("UPDATE member SET mcid=? WHERE oid=?", mcid_new, self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to set wynn binding in member table")?;
        Ok(())
    }

    /// Delete a member from db.
    /// Unlike `remove_member`, this function doesn't ensure database integrity,
    /// and also doesn't broadcast `MemberRemove` event.
    async fn remove_unchecked(&self, tx: &mut Transaction) -> Result<()> {
        info!(?self, "Removing member");
        query!("DELETE FROM member WHERE oid=?", self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to delete from member table")?;
        Ok(())
    }

    /// Given a member, unbinds all its profiles, and delete it from database
    #[instrument(skip(tx))]
    pub async fn remove(self, tx: &mut Transaction) -> Result<()> {
        let (discord, mcid) = self.links(&mut tx.exe()).await?;

        info!(?discord, ?mcid, "Removing member with following profile links");

        if mcid.is_some() {
            info!("Unbinding wynn profile");
            if self.bind_wynn(tx, None, "").await? {
                tx.signal(DBEvent::MemberRemove { mid: self, discord_id: discord, mcid });
                return Ok(());
            }
        }

        if discord.is_some() {
            info!("Unbinding discord profile");
            if self.bind_discord(tx, None).await? {
                tx.signal(DBEvent::MemberRemove { mid: self, discord_id: discord, mcid });
                return Ok(());
            }
        }

        // This should be unreachable unless above functions failed to removed empty member.
        warn!("Deleting invalid state / empty member from table");
        query!("DELETE FROM member WHERE oid=?", self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to delete from member table")?;
        tx.signal(DBEvent::MemberRemove { mid: self, discord_id: discord, mcid });
        Ok(())
    }
}

impl DiscordId {
    /// Add a new discord profile.
    ///
    /// # Preconditions
    /// The member exists and is linked to the discord id
    async fn add_profile(&self, tx: &mut Transaction, mid: Option<MemberId>) -> Result<()> {
        info!(?self, ?mid, "Creating new discord profile");
        query!("INSERT INTO discord (id,mid) VALUES (?,?)", self, mid)
            .execute(&mut tx.tx)
            .await
            .context("Failed to add discord profile")?;
        tx.signal(DBEvent::DiscordProfileAdd { discord_id: *self, mid });
        Ok(())
    }

    /// Update a discord profile's message count.
    pub async fn update_message(&self, tx: &mut Transaction, amount: i64) -> Result<()> {
        query!(
            "UPDATE discord SET message=message+?,message_week=message_week+? WHERE id=?",
            amount,
            amount,
            self
        )
        .execute(&mut tx.tx)
        .await
        .context("Failed to update discord.message and discord.message_week")?;
        Ok(())
    }

    /// Update a discord profile's voice activity.
    pub async fn update_voice(&self, tx: &mut Transaction, amount: i64) -> Result<()> {
        query!("UPDATE discord SET voice=voice+?,voice_week=voice_week+? WHERE id=?", amount, amount, self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to update discord.voice and discord.voice_week")?;
        Ok(())
    }

    /// Set a discord profile's member binding to given mid.
    /// This function doesn't ensure database integrity.
    async fn link_unchecked(&self, tx: &mut Transaction, mid: Option<MemberId>) -> Result<()> {
        info!(?mid, ?self, "Linking discord profile to member");
        query!("UPDATE discord SET mid=? WHERE id=?", mid, self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to update discord profile mid link")?;
        Ok(())
    }

    /// Set a discord profile's member binding to given mid,
    /// and if the profile doesn't exists, it is created first.
    /// This function doesn't ensure database integrity.
    #[instrument(skip(tx))]
    async fn link_or_create_unchecked(&self, tx: &mut Transaction, mid: Option<MemberId>) -> Result<()> {
        if self.exist(&mut tx.exe()).await? {
            info!("Linking to existing discord profile");
            self.link_unchecked(tx, mid).await?;
        } else {
            info!("Linking to newly created discord profile");
            self.add_profile(tx, mid).await?;
        }
        Ok(())
    }
}

impl McId {
    /// Add a new wynn profile.
    ///
    /// # Preconditions
    /// The member exists and is linked to the mcid
    async fn add_wynn_profile(&self, tx: &mut Transaction, mid: Option<MemberId>, ign: &str) -> Result<()> {
        info!(?self, ?mid, "Creating new wynn profile");
        query!("INSERT INTO wynn (id,mid,ign) VALUES (?,?,?)", self, mid, ign)
            .execute(&mut tx.tx)
            .await
            .context("Failed to add wynn profile")?;
        tx.signal.signal(DBEvent::WynnProfileAdd { mcid: self.clone(), mid });
        Ok(())
    }

    /// Add a new guild profile.
    ///
    /// # Preconditions
    /// The member exists and is linked to the mcid
    /// If a wynn profile with the same mcid exists, its `wynn.guild` is true
    async fn add_guild_profile(&self, tx: &mut Transaction, rank: GuildRank) -> Result<()> {
        info!(?self, %rank , "Creating new guild profile");
        query!("INSERT INTO guild (id,rank) VALUES (?,?) ", self, rank)
            .execute(&mut tx.tx)
            .await
            .context("Failed to add guild profile")?;
        tx.signal(DBEvent::GuildProfileAdd { mcid: self.clone(), rank });
        Ok(())
    }

    /// Update a wynn profile's online activity.
    pub async fn update_activity(&self, tx: &mut Transaction, amount: i64) -> Result<()> {
        query!(
            "UPDATE wynn SET activity=activity+?,activity_week=activity_week+? WHERE id=?",
            amount,
            amount,
            self
        )
        .execute(&mut tx.tx)
        .await
        .context("Failed to update wynn.activity and wynn.activity_week")?;
        Ok(())
    }

    /// Update a wynn profile's ign.
    pub async fn set_ign(&self, tx: &mut Transaction, ign: &str) -> Result<()> {
        info!(?self, ign, "Updating wynn ign");
        query!("UPDATE wynn SET ign=? WHERE id=?", ign, self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to update wynn.ign")?;
        Ok(())
    }

    /// Change a guild profile's guild rank.
    pub async fn set_rank(&self, tx: &mut Transaction, rank: GuildRank) -> Result<()> {
        info!(?self, %rank, "Updating guild rank");
        query!("UPDATE guild SET rank=? WHERE id=?", rank, self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to update guild.rank")?;
        Ok(())
    }

    /// Update a guild profile's xp tracking.
    pub async fn update_xp(&self, tx: &mut Transaction, amount: i64) -> Result<()> {
        info!(?self, amount, "Updating guild xp");
        query!("UPDATE guild SET xp=xp+?,xp_week=xp_week+? WHERE id=?", amount, amount, self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to update guild.xp and guild.xp_week")?;
        Ok(())
    }

    /// Set a wynn profile's member binding to given mid.
    /// This function doesn't ensure database integrity.
    async fn link_unchecked(&self, tx: &mut Transaction, mid: Option<MemberId>) -> Result<()> {
        info!(?mid, ?self, "Linking wynn profile to member");
        query!("UPDATE wynn SET mid=? WHERE id=?", mid, self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to update wynn profile mid link")?;
        Ok(())
    }

    /// Set a wynn profile's member binding to given mid,
    /// and if the profile doesn't exists, it is created first.
    /// This function doesn't ensure database integrity.
    #[instrument(skip(tx))]
    async fn link_or_create_unchecked(
        &self, tx: &mut Transaction, mid: Option<MemberId>, ign: &str,
    ) -> Result<()> {
        if self.wynn_exist(&mut tx.exe()).await? {
            info!("Linking to existing wynn profile");
            self.link_unchecked(tx, mid).await?;
        } else {
            info!("Linking to newly created wynn profile");
            self.add_wynn_profile(tx, mid, ign).await?;
        }
        Ok(())
    }

    /// Update guild status.
    /// If wynn or guild profile is missing, new one is created.
    /// If a new guild partial is created, their member id is returned.
    #[instrument(skip(tx))]
    pub async fn bind_guild(
        &self, tx: &mut Transaction, ign: &str, status: bool, rank: GuildRank,
    ) -> Result<Option<MemberId>> {
        if !self.wynn_exist(&mut tx.exe()).await? {
            info!("Adding missing wynn profile");
            self.add_wynn_profile(tx, None, ign).await?;
        }
        if !self.guild_exist(&mut tx.exe()).await? {
            info!("Adding missing guild profile");
            self.add_guild_profile(tx, rank).await?;
        }

        info!("Updating wynn.guild");
        let val = if status { 1 } else { 0 };
        query!("UPDATE wynn SET guild=? WHERE id=?", val, self)
            .execute(&mut tx.tx)
            .await
            .context("Failed to update wynn.guild")?;

        match self.mid(&mut tx.exe()).await? {
            Some(mid) => {
                // The only way a member can be affected by wynn.guild update, is if they are a guild
                // partial and wynn.guild is set to false.
                if !status {
                    if let MemberType::GuildPartial = mid.kind(&mut tx.exe()).await? {
                        info!("Removing guild partial member because guild profile is unlinked");

                        info!("Unbinding wynn profile");
                        mid.bind_wynn_unchecked(tx, None).await?;
                        self.link_unchecked(tx, None).await?;
                        tx.signal(DBEvent::WynnProfileUnbind { mid, before: self.clone(), removed: true });

                        let (discord, _) = mid.links(&mut tx.exe()).await?;
                        mid.remove_unchecked(tx).await?;
                        tx.signal(DBEvent::MemberRemove {
                            mid,
                            discord_id: discord,
                            mcid: Some(self.clone()),
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
                        self,
                        MemberType::GuildPartial,
                        member_rank
                    )
                    .execute(&mut tx.tx)
                    .await
                    .context("Failed to add guild partial member")?
                    .last_insert_rowid();
                    let mid = MemberId(mid);

                    self.link_unchecked(tx, Some(mid)).await?;

                    tx.signal(DBEvent::WynnProfileBind { mid, old: None, new: self.clone() });
                    tx.signal(DBEvent::MemberAdd {
                        mid,
                        discord_id: None,
                        mcid: Some(self.clone()),
                        rank: member_rank,
                    });
                    return Ok(Some(mid));
                }
            }
        }

        Ok(None)
    }
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
