//! Functions for fetching data from the database
use std::str::FromStr;

use anyhow::{Context, Result};
use sqlx::{query, query_as};

use crate::model::discord::{DiscordId, DiscordProfile, DiscordProfileRow};
use crate::model::guild::{GuildProfile, GuildProfileRow, GuildRank};
use crate::model::member::{Member, MemberId, MemberRank, MemberRow, MemberType};
use crate::model::wynn::{McId, WynnProfile, WynnProfileRow};
use crate::Executor;

impl MemberId {
    pub async fn get(&self, exe: &mut Executor<'_>) -> Result<Option<Member>> {
        let member = exe
            .optional(query_as!(
                MemberRow,
                "SELECT oid AS id,type AS member_type,discord,mcid,rank FROM member WHERE oid=?",
                self
            ))
            .await
            .context("Failed to fetch member")?;
        match member {
            Some(member) => Ok(Some(Member::from_row(member)?)),
            None => Ok(None),
        }
    }

    pub async fn exist(&self, exe: &mut Executor<'_>) -> Result<bool> {
        Ok(exe
            .optional(query!("SELECT oid FROM member WHERE oid=?", self))
            .await
            .context("Failed check if member exists")?
            .is_some())
    }

    //     pub async fn discord(&self, exe: &mut Executor<'_>) -> Result<Option<DiscordId>> {
    //         let row = exe
    //             .one(query!("SELECT discord FROM member WHERE oid=?", self))
    //             .await
    //             .context("Failed to fetch member.discord")?;
    //         Ok(row.discord)
    //     }

    //     pub async fn mcid(&self, exe: &mut Executor<'_>) -> Result<Option<McId>> {
    //         let row = exe
    //             .one(query!("SELECT mcid FROM member WHERE oid=?", self))
    //             .await
    //             .context("Failed to fetch member.mcid")?;
    //         Ok(row.mcid)
    //     }

    pub async fn links(&self, exe: &mut Executor<'_>) -> Result<(Option<DiscordId>, Option<McId>)> {
        let row = exe
            .one(query!("SELECT discord,mcid FROM member WHERE oid=?", self))
            .await
            .context("Failed to fetch from member table")?;
        Ok((row.discord.map(|id| DiscordId(id)), row.mcid.map(|id| McId(id))))
    }

    /// Get member type
    pub async fn kind(&self, exe: &mut Executor<'_>) -> Result<MemberType> {
        let row = exe
            .one(query!("SELECT type AS member_type FROM member WHERE oid=?", self))
            .await
            .context("Failed to fetch member.type")?;
        Ok(MemberType::from_str(&row.member_type)?)
    }

    /// Get member rank
    pub async fn rank(&self, exe: &mut Executor<'_>) -> Result<MemberRank> {
        let rank = exe
            .one(query!("SELECT rank FROM member WHERE oid=?", self))
            .await
            .context("Failed to get member.rank")?
            .rank;
        MemberRank::decode(&rank)
    }
}

impl DiscordId {
    /// Get the entire profile
    pub async fn get(&self, exe: &mut Executor<'_>) -> Result<Option<DiscordProfile>> {
        let discord = exe
            .optional(query_as!(DiscordProfileRow, "SELECT * FROM discord WHERE id=?", self))
            .await
            .context("Failed to fetch discord profile")?;
        match discord {
            Some(discord) => Ok(Some(DiscordProfile::from_row(discord)?)),
            None => Ok(None),
        }
    }

    /// Get linked member id
    pub async fn mid(&self, exe: &mut Executor<'_>) -> Result<Option<MemberId>> {
        let row = exe
            .optional(query!("SELECT mid FROM discord WHERE id=?", self))
            .await
            .context("Failed to fetch discord.mid")?;
        if let Some(row) = row {
            if let Some(mid) = row.mid {
                return Ok(Some(MemberId(mid)));
            }
        }
        Ok(None)
    }

    /// Check if corresponding profile exists.
    pub async fn exist(&self, exe: &mut Executor<'_>) -> Result<bool> {
        Ok(exe
            .optional(query!("SELECT id FROM discord WHERE id=?", self))
            .await
            .context("Failed to check if discord profile exists")?
            .is_some())
    }
}

impl McId {
    /// Get the entire profile.
    pub async fn get_wynn(&self, exe: &mut Executor<'_>) -> Result<Option<WynnProfile>> {
        let wynn = exe
            .optional(query_as!(WynnProfileRow, "SELECT * FROM wynn WHERE id=?", self))
            .await
            .context("Failed to fetch from wynn table")?;
        Ok(wynn.map(|row| WynnProfile::from_row(row)))
    }

    /// Get the entire guild profile
    pub async fn get_guild(&self, exe: &mut Executor<'_>) -> Result<Option<GuildProfile>> {
        let guild = exe
            .optional(query_as!(GuildProfileRow, "SELECT * FROM guild WHERE id=?", self))
            .await
            .context("Failed to fetch from guild table")?;
        match guild {
            Some(guild) => Ok(Some(GuildProfile::from_row(exe, guild).await?)),
            None => Ok(None),
        }
    }

    /// Get ign's mcid
    pub async fn from_ign(exe: &mut Executor<'_>, ign: &str) -> Result<Option<Self>> {
        let row = exe
            .optional(query!("SELECT id FROM wynn WHERE ign=?", ign))
            .await
            .context("Failed to fetch wynn.id")?;

        Ok(row.map(|row| Self(row.id)))
    }

    /// Get linked member id
    pub async fn mid(&self, exe: &mut Executor<'_>) -> Result<Option<MemberId>> {
        let row = exe
            .optional(query!("SELECT mid FROM wynn WHERE id=?", self))
            .await
            .context("Failed to fetch wynn.mid")?;
        if let Some(row) = row {
            if let Some(mid) = row.mid {
                return Ok(Some(MemberId(mid)));
            }
        }
        Ok(None)
    }

    /// Get an ign's linked member id
    pub async fn mid_from_ign(exe: &mut Executor<'_>, ign: &str) -> Result<Option<MemberId>> {
        Ok(exe
            .one(query!("SELECT mid FROM wynn WHERE ign=?", ign))
            .await
            .context("Failed to fetch wynn.mid from ign")?
            .mid
            .map(|mid| MemberId(mid)))
    }

    /// Get ign
    pub async fn ign(&self, exe: &mut Executor<'_>) -> Result<String> {
        Ok(exe
            .one(query!("SELECT ign FROM wynn WHERE id=?", self))
            .await
            .context("Failed to get wynn.ign")?
            .ign)
    }

    /// Check if corresponding wynn profile exists.
    pub async fn wynn_exist(&self, exe: &mut Executor<'_>) -> Result<bool> {
        Ok(exe
            .optional(query!("SELECT id FROM wynn WHERE id=?", self))
            .await
            .context("Failed to fetch from wynn table")?
            .is_some())
    }

    /// Check if is is in the in-game guild
    pub async fn in_guild(&self, exe: &mut Executor<'_>) -> Result<bool> {
        Ok(exe
            .optional(query!("SELECT guild FROM wynn WHERE id=?", self))
            .await
            .context("Failed to fetch wynn.guild")?
            .map(|n| n.guild > 0)
            .unwrap_or(false))
    }

    /// Check if corresponding guild profile exists
    pub async fn guild_exist(&self, exe: &mut Executor<'_>) -> Result<bool> {
        Ok(exe
            .optional(query!("SELECT id FROM guild WHERE id=?", self))
            .await
            .context("Failed to select from guild table")?
            .is_some())
    }

    /// Get guild rank
    pub async fn rank(&self, exe: &mut Executor<'_>) -> Result<GuildRank> {
        let rank = exe
            .one(query!("SELECT rank FROM guild WHERE id=?", self))
            .await
            .context("Failed to get guild.rank")?
            .rank;
        Ok(GuildRank::from_str(&rank)?)
    }

    /// Get a guild contributed xp
    pub async fn xp(&self, exe: &mut Executor<'_>) -> Result<i64> {
        Ok(exe
            .one(query!("SELECT xp FROM guild WHERE id=?", self))
            .await
            .context("Failed to get guild.xp")?
            .xp)
    }
}
