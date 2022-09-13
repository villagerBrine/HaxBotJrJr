use std::str::FromStr;

use anyhow::{bail, Result};

use util::{ioerr, ok, ok_some};

use crate::model::discord::{DiscordId, DiscordProfile};
use crate::model::guild::GuildProfile;
use crate::model::member::{Member, MemberId};
use crate::model::wynn::{McId, WynnProfile};
use crate::utils;
use crate::DB;

#[derive(Debug)]
/// Types of profiles
pub enum ProfileType {
    Discord,
    Wynn,
    Guild,
}

impl FromStr for ProfileType {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "discord" => Ok(Self::Discord),
            "wynn" => Ok(Self::Wynn),
            "guild" => Ok(Self::Guild),
            _ => ioerr!("Failed to parse '{}' as ProfileType", s),
        }
    }
}

#[derive(Debug)]
/// Database profiles
pub struct Profiles {
    pub member: Option<Member>,
    pub guild: Option<GuildProfile>,
    pub discord: Option<DiscordProfile>,
    pub wynn: Option<WynnProfile>,
}

impl Profiles {
    /// Checks if there are no profiles
    pub fn is_none(&self) -> bool {
        self.member.is_none() && self.guild.is_none() && self.discord.is_none() && self.wynn.is_none()
    }

    /// Get profiles related to the member id
    pub async fn from_member(db: &DB, mid: MemberId) -> Self {
        match mid.get(&mut db.exe()).await {
            Ok(Some(member)) => {
                let discord = match member.discord {
                    Some(id) => ok!(id.get(&mut db.exe()).await, None),
                    None => None,
                };
                let (wynn, guild) = utils::get_wynn_guild_profiles(db, &member.mcid).await;
                Self {
                    member: Some(member),
                    guild,
                    discord,
                    wynn,
                }
            }
            _ => Self {
                member: None,
                guild: None,
                discord: None,
                wynn: None,
            },
        }
    }

    /// Get profiles related to the discord id
    pub async fn from_discord(db: &DB, discord_id: DiscordId) -> Self {
        match discord_id.get(&mut db.exe()).await {
            Ok(Some(discord)) => {
                // Checks if the discord is linked with a member
                if let Some(mid) = discord.mid {
                    if let Ok(Some(member)) = mid.get(&mut db.exe()).await {
                        let (wynn, guild) = utils::get_wynn_guild_profiles(db, &member.mcid).await;
                        return Self {
                            member: Some(member),
                            guild,
                            discord: Some(discord),
                            wynn,
                        };
                    }
                }
                Self {
                    member: None,
                    guild: None,
                    discord: Some(discord),
                    wynn: None,
                }
            }
            _ => Self {
                member: None,
                guild: None,
                discord: None,
                wynn: None,
            },
        }
    }

    /// Get profiles related to the mcid
    pub async fn from_mc(db: &DB, mcid: &McId) -> Self {
        let (wynn, guild) = utils::get_wynn_guild_profiles(db, &Some(mcid.clone())).await;
        let (member, discord) = match wynn {
            Some(WynnProfile { mid: Some(mid), .. }) => match mid.get(&mut db.exe()).await {
                Ok(Some(member)) => match member.discord {
                    Some(discord_id) => match discord_id.get(&mut db.exe()).await {
                        Ok(Some(discord)) => (Some(member), Some(discord)),
                        _ => (Some(member), None),
                    },
                    _ => (Some(member), None),
                },
                _ => (None, None),
            },
            _ => (None, None),
        };
        Self { wynn, guild, member, discord }
    }
}

#[derive(Debug)]
/// All ids that are related to the database
pub struct Ids {
    pub member: Option<MemberId>,
    pub mc: Option<McId>,
    pub discord: Option<DiscordId>,
}

impl Ids {
    /// Fetch the profiles related to the id
    pub async fn to_profiles(&self, db: &DB) -> Profiles {
        let member = match self.member {
            Some(mid) => ok_some!(mid.get(&mut db.exe()).await),
            None => None,
        };
        let guild = match &self.mc {
            Some(mcid) => ok_some!(mcid.get_guild(&mut db.exe()).await),
            None => None,
        };
        let wynn = match &self.mc {
            Some(mcid) => ok_some!(mcid.get_wynn(&mut db.exe()).await),
            None => None,
        };
        let discord = match self.discord {
            Some(discord) => ok_some!(discord.get(&mut db.exe()).await),
            None => None,
        };
        Profiles { member, guild, discord, wynn }
    }

    /// Get all ids related to the member
    pub async fn from_member(db: &DB, mid: MemberId) -> Self {
        match mid.exist(&mut db.exe()).await.ok() {
            Some(exist) => {
                if exist {
                    let (discord, mc) = ok!(mid.links(&mut db.exe()).await, (None, None));
                    Self { member: Some(mid), discord, mc }
                } else {
                    Self {
                        member: Some(mid),
                        mc: None,
                        discord: None,
                    }
                }
            }
            None => Self { member: None, mc: None, discord: None },
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
/// Represent database columns that can be selected.
pub enum Column {
    // Discord
    DMessage,
    DWeeklyMessage,
    DVoice,
    DWeeklyVoice,
    // Wynn
    WGuild,
    WIgn,
    WOnline,
    WWeeklyOnline,
    WAvgOnline,
    // Guild
    GRank,
    GXp,
    GWeeklyXp,
    // Member
    MId,
    MMcid,
    MDiscord,
    MRank,
    MType,
}

impl Column {
    /// Return which profile table a column belongs to, None if it is part of the member table
    pub fn profile(&self) -> Option<ProfileType> {
        match self {
            Self::MId | Self::MRank | Self::MType | Self::MMcid | Self::MDiscord => None,
            Self::GRank | Self::GXp | Self::GWeeklyXp => Some(ProfileType::Guild),
            Self::WGuild | Self::WIgn | Self::WOnline | Self::WWeeklyOnline | Self::WAvgOnline => {
                Some(ProfileType::Wynn)
            }
            _ => Some(ProfileType::Discord),
        }
    }

    /// Get the column name within the database
    pub fn name(&self) -> &str {
        match self {
            Self::MDiscord => "discord",
            Self::MMcid => "mcid",
            Self::MId => "oid",
            Self::DMessage => "message",
            Self::DWeeklyMessage => "message_week",
            Self::DVoice => "voice",
            Self::DWeeklyVoice => "voice_week",
            Self::WGuild => "guild",
            Self::WIgn => "ign",
            Self::WOnline => "activity",
            Self::WWeeklyOnline => "activity_week",
            Self::WAvgOnline => "activity_avg",
            Self::GRank | Self::MRank => "rank",
            Self::GXp => "xp",
            Self::GWeeklyXp => "xp_week",
            Self::MType => "type",
        }
    }
}

impl FromStr for Column {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "discord_id" => Self::MDiscord,
            "message" => Self::DMessage,
            "weekly_message" => Self::DWeeklyMessage,
            "voice" => Self::DVoice,
            "weekly_voice" => Self::DWeeklyVoice,
            "mc_id" => Self::MMcid,
            "in_guild" => Self::WGuild,
            "ign" => Self::WIgn,
            "online" => Self::WOnline,
            "weekly_online" => Self::WWeeklyOnline,
            "avg_online" => Self::WAvgOnline,
            "guild_rank" => Self::GRank,
            "xp" => Self::GXp,
            "weekly_xp" => Self::GWeeklyXp,
            "id" => Self::MId,
            "rank" => Self::MRank,
            "type" => Self::MType,
            _ => return ioerr!("Failed to parse '{}' as Column", s),
        })
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
/// All tracked stat columns
pub enum Stat {
    Message,
    WeeklyMessage,
    Voice,
    WeeklyVoice,
    Online,
    WeeklyOnline,
    AvgOnline,
    Xp,
    WeeklyXp,
}

impl Stat {
    /// Convert from `Column`
    pub fn from_column(col: &Column) -> Option<Self> {
        Some(match col {
            Column::DMessage => Self::Message,
            Column::DWeeklyMessage => Self::WeeklyMessage,
            Column::DVoice => Self::Voice,
            Column::DWeeklyVoice => Self::WeeklyVoice,
            Column::WOnline => Self::Online,
            Column::WWeeklyOnline => Self::WeeklyOnline,
            Column::WAvgOnline => Self::AvgOnline,
            Column::GXp => Self::Xp,
            Column::GWeeklyXp => Self::WeeklyXp,
            _ => return None,
        })
    }

    /// Convert to `Column`
    pub fn to_column(&self) -> Column {
        match self {
            Self::Message => Column::DMessage,
            Self::WeeklyMessage => Column::DWeeklyMessage,
            Self::Voice => Column::DVoice,
            Self::WeeklyVoice => Column::DWeeklyVoice,
            Self::Online => Column::WOnline,
            Self::WeeklyOnline => Column::WWeeklyOnline,
            Self::AvgOnline => Column::WAvgOnline,
            Self::Xp => Column::GXp,
            Self::WeeklyXp => Column::GWeeklyXp,
        }
    }

    /// Parse string into stat value based on stat type
    pub fn parse_val(&self, val: &str) -> Result<u64> {
        match self {
            // parse as time duration
            Self::Voice | Self::WeeklyVoice | Self::Online | Self::WeeklyOnline | Self::AvgOnline => {
                util::string::parse_second(val)
            }
            // parse as number
            _ => match u64::try_from(util::string::parse_num(val)?) {
                Ok(n) => Ok(n),
                Err(_) => bail!("Number can't be negative"),
            },
        }
    }
}

impl FromStr for Stat {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(col) = Column::from_str(s) {
            if let Some(stat) = Self::from_column(&col) {
                return Ok(stat);
            }
        }
        ioerr!("Failed to parse '{}' as Stat", s)
    }
}
