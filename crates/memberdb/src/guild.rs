use std::{fmt, str::FromStr};

use anyhow::Result;

use util::impl_sqlx_type;

use crate::DB;
use crate::member::{MemberRank, MemberId};
use crate::wynn::McId;

#[derive(Debug, Copy, Clone)]
pub enum GuildRank {Owner, Chief, Strategist, Captain, Recruiter, Recruit}

impl_sqlx_type!(GuildRank);

impl GuildRank {
    pub fn to_member_rank(&self) -> MemberRank {
        match self {
            Self::Owner => MemberRank::One,
            Self::Chief => MemberRank::Two,
            Self::Strategist => MemberRank::Three,
            Self::Captain => MemberRank::Four,
            Self::Recruiter => MemberRank::Five,
            Self::Recruit => MemberRank::Six
        }
    }

    pub fn from_api(rank: &str) -> Result<GuildRank> {
        match rank {
            "OWNER" => Ok(Self::Owner),
            "CHIEF" => Ok(Self::Chief),
            "STRATEGIST" => Ok(Self::Strategist),
            "CAPTAIN" => Ok(Self::Captain),
            "RECRUITER" => Ok(Self::Recruiter),
            "RECRUIT" => Ok(Self::Recruit),
            _ => Err(ParseGuildRankError(rank.to_string()).into())
        }
    }

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
            Self::Recruit => write!(f, "Recruit")
        }
    }
}

#[derive(Debug)]
pub struct ParseGuildRankError(String);

impl std::error::Error for ParseGuildRankError {}

impl fmt::Display for ParseGuildRankError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to parse '{}' as GuildRank", self.0)
    }
}

impl FromStr for GuildRank {
    type Err = ParseGuildRankError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Owner" => Ok(Self::Owner),
            "Chief" => Ok(Self::Chief),
            "Strategist" => Ok(Self::Strategist),
            "Captain" => Ok(Self::Captain),
            "Recruiter" => Ok(Self::Recruiter),
            "Recruit" => Ok(Self::Recruit),
            _ => Err(ParseGuildRankError(s.to_string()))
        }
    }
}

#[derive(Debug)]
pub struct GuildProfileRow {
    pub id: McId,
    pub rank: String,
    pub xp: i64,
    pub xp_week: i64
}

#[derive(Debug)]
pub struct GuildProfile {
    pub id: McId,
    pub mid: Option<MemberId>,
    pub rank: GuildRank,
    pub xp: i64,
    pub xp_week: i64
}

impl GuildProfile {
    pub async fn from_row(db: &DB, row: GuildProfileRow) -> Result<Self> {
        let rank = GuildRank::from_str(&row.rank)?;
        let mid = super::get_wynn_mid(db, &row.id).await?;
        Ok(Self {id: row.id, mid, rank,
                 xp: row.xp, xp_week: row.xp_week})
    }
}
