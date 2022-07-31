//! Models for the guild table
use std::{fmt, str::FromStr};

use anyhow::Result;

use util::{impl_sqlx_type, ioerr};

use crate::model::member::{MemberId, MemberRank};
use crate::model::wynn::McId;
use crate::DB;

#[derive(Debug, Copy, Clone)]
/// In-game guild ranks
pub enum GuildRank {
    Owner,
    Chief,
    Strategist,
    Captain,
    Recruiter,
    Recruit,
}

impl_sqlx_type!(GuildRank);

impl GuildRank {
    /// Get the corresponding member rank
    pub fn to_member_rank(&self) -> MemberRank {
        match self {
            Self::Owner => MemberRank::One,
            Self::Chief => MemberRank::Two,
            Self::Strategist => MemberRank::Three,
            Self::Captain => MemberRank::Four,
            Self::Recruiter => MemberRank::Five,
            Self::Recruit => MemberRank::Six,
        }
    }

    /// Convert from all uppercase string, as appeared in the wynncraft api.
    pub fn from_api(rank: &str) -> Result<GuildRank> {
        match rank {
            "OWNER" => Ok(Self::Owner),
            "CHIEF" => Ok(Self::Chief),
            "STRATEGIST" => Ok(Self::Strategist),
            "CAPTAIN" => Ok(Self::Captain),
            "RECRUITER" => Ok(Self::Recruiter),
            "RECRUIT" => Ok(Self::Recruit),
            _ => ioerr!("Failed to convert '{}' as GuildRank", rank),
        }
    }

    /// Convert to all uppercase string, as appeared in the wynncraft api.
    pub fn to_api(&self) -> String {
        self.to_string().to_uppercase()
    }
}

impl fmt::Display for GuildRank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Owner => write!(f, "Owner"),
            Self::Chief => write!(f, "Chief"),
            Self::Strategist => write!(f, "Strategist"),
            Self::Captain => write!(f, "Captain"),
            Self::Recruiter => write!(f, "Recruiter"),
            Self::Recruit => write!(f, "Recruit"),
        }
    }
}

impl FromStr for GuildRank {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Owner" => Ok(Self::Owner),
            "Chief" => Ok(Self::Chief),
            "Strategist" => Ok(Self::Strategist),
            "Captain" => Ok(Self::Captain),
            "Recruiter" => Ok(Self::Recruiter),
            "Recruit" => Ok(Self::Recruit),
            _ => ioerr!("Failed to parse '{}' as GuildRank", s),
        }
    }
}

#[derive(Debug)]
/// Guild table model with database primitives.
/// Use this to query entire guild profile from database, and convert it to `GuildProfile` with more
/// convenient field values.
pub struct GuildProfileRow {
    pub id: McId,
    pub rank: String,
    pub xp: i64,
    pub xp_week: i64,
}

#[derive(Debug)]
/// Guild table model.
/// This can't be used to query entire guil profile from database, instead query one using
/// `GuildProfileRow`, and then convert it to `GuildProfile`.
pub struct GuildProfile {
    pub id: McId,
    pub mid: Option<MemberId>,
    pub rank: GuildRank,
    pub xp: i64,
    pub xp_week: i64,
}

impl GuildProfile {
    /// Convert from `GuildProfileRow`
    pub async fn from_row(db: &DB, row: GuildProfileRow) -> Result<Self> {
        let rank = GuildRank::from_str(&row.rank)?;
        let mid = crate::get_wynn_mid(db, &row.id).await?;
        Ok(Self {
            id: row.id,
            mid,
            rank,
            xp: row.xp,
            xp_week: row.xp_week,
        })
    }
}
