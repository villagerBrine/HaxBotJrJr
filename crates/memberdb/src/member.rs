use std::fmt;
use std::str::FromStr;

use anyhow::Result;
use serenity::model::guild::{Guild, Role};

use util::impl_sqlx_type;

use crate::discord::DiscordId;
use crate::error::{ParseMemberRankError, ParseMemberTypeError, ParseProfileTypeError};
use crate::wynn::McId;

pub type MemberId = i64;

#[derive(sqlx::Type, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemberRank {
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
}

pub const MEMBER_RANKS: [MemberRank; 7] = [
    MemberRank::Zero,
    MemberRank::One,
    MemberRank::Two,
    MemberRank::Three,
    MemberRank::Four,
    MemberRank::Five,
    MemberRank::Six,
];
pub const MANAGED_MEMBER_RANKS: [MemberRank; 5] = [
    MemberRank::Two,
    MemberRank::Three,
    MemberRank::Four,
    MemberRank::Five,
    MemberRank::Six,
];
pub const INIT_MEMBER_RANK: MemberRank = MemberRank::Six;
pub const MRANK_ZERO_STR: &str = "Founder";
pub const MRANK_ONE_STR: &str = "Commander";
pub const MRANK_TWO_STR: &str = "Cosmonaut";
pub const MRANK_THREE_STR: &str = "Architect";
pub const MRANK_FOUR_STR: &str = "Pilot";
pub const MRANK_FIVE_STR: &str = "Rocketeer";
pub const MRANK_SIX_STR: &str = "Cadet";

impl MemberRank {
    pub fn promote(&self) -> Option<Self> {
        if let Some(i) = MEMBER_RANKS.iter().position(|r| r == self) {
            if i > 0 {
                return Some(MEMBER_RANKS[i - 1]);
            }
        }
        None
    }

    pub fn demote(&self) -> Option<Self> {
        if let Some(i) = MEMBER_RANKS.iter().position(|r| r == self) {
            if i < MEMBER_RANKS.len() - 1 {
                return Some(MEMBER_RANKS[i + 1]);
            }
        }
        None
    }

    pub fn get_role<'a>(&self, guild: &'a Guild) -> Option<&'a Role> {
        guild.role_by_name(&self.to_string())
    }

    pub fn get_group_role<'a>(&self, guild: &'a Guild) -> Option<&'a Role> {
        guild.role_by_name(self.get_group_name())
    }

    pub fn is_same_group(&self, other: Self) -> bool {
        self.get_group_name() == other.get_group_name()
    }

    pub fn get_group_name(&self) -> &str {
        match self {
            Self::Zero | Self::One | Self::Two => "Mission Specialist",
            Self::Three | Self::Four => "Flight Captains",
            Self::Five | Self::Six => "Passengers",
        }
    }

    pub fn from_role(&self, role: &Role) -> Option<Self> {
        Self::from_str(&role.name).ok()
    }

    pub fn decode(s: &str) -> Result<Self> {
        Ok(match s {
            "Zero" => Self::Zero,
            "One" => Self::One,
            "Two" => Self::Two,
            "Three" => Self::Three,
            "Four" => Self::Four,
            "Five" => Self::Five,
            "Six" => Self::Six,
            s => return Err(ParseMemberRankError(s.to_string()).into()),
        })
    }

    pub fn get_symbol(&self) -> char {
        match self {
            Self::Zero => '❂',
            Self::One => '❂',
            Self::Two => '❈',
            Self::Three => '✾',
            Self::Four => '✲',
            Self::Five => '✮',
            Self::Six => '✧',
        }
    }
}

impl fmt::Display for MemberRank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => write!(f, "{}", MRANK_ZERO_STR),
            Self::One => write!(f, "{}", MRANK_ONE_STR),
            Self::Two => write!(f, "{}", MRANK_TWO_STR),
            Self::Three => write!(f, "{}", MRANK_THREE_STR),
            Self::Four => write!(f, "{}", MRANK_FOUR_STR),
            Self::Five => write!(f, "{}", MRANK_FIVE_STR),
            Self::Six => write!(f, "{}", MRANK_SIX_STR),
        }
    }
}

impl FromStr for MemberRank {
    type Err = ParseMemberRankError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            MRANK_ZERO_STR => Ok(Self::Zero),
            MRANK_ONE_STR => Ok(Self::One),
            MRANK_TWO_STR => Ok(Self::Two),
            MRANK_THREE_STR => Ok(Self::Three),
            MRANK_FOUR_STR => Ok(Self::Four),
            MRANK_FIVE_STR => Ok(Self::Five),
            MRANK_SIX_STR => Ok(Self::Six),
            _ => Err(ParseMemberRankError(s.to_string())),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum MemberType {
    Full,
    DiscordPartial,
    WynnPartial,
    GuildPartial,
}

impl_sqlx_type!(MemberType);

impl MemberType {
    pub fn is_full(&self) -> bool {
        if let Self::Full = self {
            return true;
        }
        false
    }

    pub fn is_partial(&self) -> bool {
        !self.is_full()
    }
}

impl fmt::Display for MemberType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "full"),
            Self::GuildPartial => write!(f, "guild"),
            Self::DiscordPartial => write!(f, "discord"),
            Self::WynnPartial => write!(f, "wynn"),
        }
    }
}

impl FromStr for MemberType {
    type Err = ParseMemberTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "full" => Ok(Self::Full),
            "guild" => Ok(Self::GuildPartial),
            "discord" => Ok(Self::DiscordPartial),
            "wynn" => Ok(Self::WynnPartial),
            _ => Err(ParseMemberTypeError(s.to_string())),
        }
    }
}

#[derive(Debug)]
pub enum ProfileType {
    Discord,
    Wynn,
    Guild,
}

impl FromStr for ProfileType {
    type Err = ParseProfileTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "discord" => Ok(Self::Discord),
            "wynn" => Ok(Self::Wynn),
            "guild" => Ok(Self::Guild),
            unknown => Err(ParseProfileTypeError(unknown.to_string())),
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct MemberRow {
    pub id: MemberId,
    pub discord: Option<DiscordId>,
    pub mcid: Option<McId>,
    pub member_type: String,
    pub rank: String,
}

#[derive(Debug)]
pub struct Member {
    pub id: MemberId,
    pub discord: Option<DiscordId>,
    pub mcid: Option<McId>,
    pub member_type: MemberType,
    pub rank: MemberRank,
}

impl Member {
    pub fn from_row(row: MemberRow) -> Result<Member> {
        let rank = MemberRank::decode(&row.rank)?;
        let member_type = MemberType::from_str(&row.member_type)?;
        Ok(Member {
            id: row.id,
            discord: row.discord,
            mcid: row.mcid,
            member_type,
            rank,
        })
    }
}
