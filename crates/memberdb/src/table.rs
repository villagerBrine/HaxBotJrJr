use std::cmp::Ordering;
use std::io;
use std::str::FromStr;

use anyhow::{bail, Result};
use serenity::client::Cache;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use util::some;

use crate::error::ParseMemberFilterError;
use crate::guild::GuildRank;
use crate::member::{MemberRank, MemberType};
use crate::DB;

pub enum Stat {
    Message(bool),
    Voice(bool),
    Online(bool),
    Xp(bool),
}

impl Stat {
    pub fn table(&self) -> &str {
        match self {
            Self::Message(_) | Self::Voice(_) => "discord",
            Self::Online(_) => "wynn",
            Self::Xp(_) => "guild",
        }
    }

    pub fn column(&self) -> &str {
        match self {
            // Bad formatter :(
            Self::Message(week) => {
                if *week {
                    "message_week"
                } else {
                    "message"
                }
            }
            Self::Voice(week) => {
                if *week {
                    "voice_week"
                } else {
                    "voice"
                }
            }
            Self::Online(week) => {
                if *week {
                    "activity_week"
                } else {
                    "activity"
                }
            }
            Self::Xp(week) => {
                if *week {
                    "xp_week"
                } else {
                    "xp"
                }
            }
        }
    }

    pub fn display_stat(&self, stat: i64) -> String {
        match self {
            Self::Voice(_) | Self::Online(_) => util::string::fmt_second(stat),
            _ => util::string::fmt_num(stat, true),
        }
    }
}

impl FromStr for Stat {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (is_week, s) = if s.starts_with("weekly_") { (true, &s[7..]) } else { (false, s) };
        Ok(match s {
            "message" => Self::Message(is_week),
            "voice" => Self::Voice(is_week),
            "online" => Self::Online(is_week),
            "xp" => Self::Xp(is_week),
            _ => return Err(io::Error::new(io::ErrorKind::Other, "Failed to parse Stat from string")),
        })
    }
}

pub enum MemberFilter {
    Partial,
    MemberType(MemberType),
    MemberRank(MemberRank, Ordering),
    GuildRank(GuildRank, Ordering),
}

impl MemberFilter {
    pub fn where_clause(&self) -> Option<&str> {
        // format! would be more compact for some of the cases, but I really wants this function to
        // return &str instead String.
        match self {
            Self::Partial => Some("type!='full'"),
            Self::MemberType(ty) => match ty {
                MemberType::Full => Some("type='full'"),
                MemberType::DiscordPartial => Some("type='discord'"),
                MemberType::GuildPartial => Some("type='guild'"),
                MemberType::WynnPartial => Some("type='wynn'"),
            },
            Self::MemberRank(rank, order) => match order {
                Ordering::Less => match rank {
                    MemberRank::Zero => Some("rank!='Zero'"),
                    MemberRank::One => Some("rank NOT IN ('Zero','One')"),
                    MemberRank::Two => Some("rank NOT IN ('Zero','One','Two')"),
                    MemberRank::Three => Some("rank IN ('Four','Five','Six')"),
                    MemberRank::Four => Some("rank IN ('Five','Six')"),
                    MemberRank::Five => Some("rank='Six'"),
                    MemberRank::Six => None,
                },
                Ordering::Equal => match rank {
                    MemberRank::Zero => Some("rank='Zero'"),
                    MemberRank::One => Some("rank='One'"),
                    MemberRank::Two => Some("rank='Two'"),
                    MemberRank::Three => Some("rank='Three'"),
                    MemberRank::Four => Some("rank='Four'"),
                    MemberRank::Five => Some("rank='Five'"),
                    MemberRank::Six => Some("rank='Six'"),
                },
                Ordering::Greater => match rank {
                    MemberRank::Zero => None,
                    MemberRank::One => Some("rank='Zero'"),
                    MemberRank::Two => Some("rank IN ('Zero','One')"),
                    MemberRank::Three => Some("rank IN ('Zero','One','Two')"),
                    MemberRank::Four => Some("rank NOT IN ('Four','Five','Six')"),
                    MemberRank::Five => Some("rank NOT IN ('Five','Six')"),
                    MemberRank::Six => Some("rank!='Six'"),
                },
            },
            Self::GuildRank(rank, order) => match order {
                Ordering::Less => match rank {
                    GuildRank::Owner => Some("guild_rank!='Owner'"),
                    GuildRank::Chief => Some("guild_rank NOT IN ('Owner','Chief')"),
                    GuildRank::Strategist => Some("guild_rank NOT IN ('Owner','Chief','Strategist')"),
                    GuildRank::Captain => Some("guild_rank IN ('Recruiter','Recruit')"),
                    GuildRank::Recruiter => Some("guild_rank='Recruit'"),
                    GuildRank::Recruit => None,
                },
                Ordering::Equal => match rank {
                    GuildRank::Owner => Some("guild_rank='Owner'"),
                    GuildRank::Chief => Some("guild_rank='Chief'"),
                    GuildRank::Strategist => Some("guild_rank='Strategist'"),
                    GuildRank::Captain => Some("guild_rank='Captain'"),
                    GuildRank::Recruiter => Some("guild_rank='Recruiter'"),
                    GuildRank::Recruit => Some("guild_rank='Recruit'"),
                },
                Ordering::Greater => match rank {
                    GuildRank::Owner => None,
                    GuildRank::Chief => Some("guild_rank='Owner'"),
                    GuildRank::Strategist => Some("guild_rank IN ('Owner','Chief')"),
                    GuildRank::Captain => Some("guild_rank IN ('Owner','Chief','Strategist')"),
                    GuildRank::Recruiter => Some("guild_rank NOT IN ('Recruiter','Recruit')"),
                    GuildRank::Recruit => Some("guild_rank!='Recruit'"),
                },
            },
        }
    }

    pub fn guild_rank_select(&self) -> Option<&str> {
        match self {
            Self::GuildRank(..) => Some("(SELECT rank FROM guild WHERE id=member.mcid) AS guild_rank"),
            _ => None,
        }
    }
}

fn extract_ordering(s: &str) -> (Ordering, &str) {
    let symbol = s.chars().next().unwrap();
    match symbol {
        '<' => (Ordering::Less, &s[1..]),
        '>' => (Ordering::Greater, &s[1..]),
        _ => (Ordering::Equal, &s),
    }
}

impl FromStr for MemberFilter {
    type Err = ParseMemberFilterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_empty() {
            if s == "partial" {
                return Ok(Self::Partial);
            }
            if let Ok(member_type) = MemberType::from_str(s) {
                return Ok(Self::MemberType(member_type));
            }

            let (order, s) = extract_ordering(s);
            if let Ok(rank) = MemberRank::from_str(s) {
                return Ok(Self::MemberRank(rank, order));
            }
            if let Ok(rank) = GuildRank::from_str(s) {
                return Ok(Self::GuildRank(rank, order));
            }
        }
        Err(ParseMemberFilterError(s.to_string()))
    }
}

const M_SELECT: &str = "SELECT \
rank,discord,\
(SELECT ign FROM wynn WHERE mid=member.oid) AS ign";

const M_ORDER: &str = "ORDER BY ign NULLS LAST";

pub async fn list_members(cache: &Cache, db: &DB, filter: Option<MemberFilter>) -> Result<Vec<Vec<String>>> {
    Ok(sqlx::query(&match filter {
        Some(filter) => match filter.where_clause() {
            Some(clause) => match filter.guild_rank_select() {
                Some(sel) => format!("{},{} FROM member WHERE {} {}", M_SELECT, sel, clause, M_ORDER),
                None => format!("{} FROM member WHERE {} {}", M_SELECT, clause, M_ORDER),
            },
            None => bail!("Invalid filter"),
        },
        None => format!("{} FROM member {}", M_SELECT, M_ORDER),
    })
    .map(|r: SqliteRow| {
        vec![
            some!(r.get("ign"), "".to_string()),
            match r.get::<Option<i64>, &str>("discord").map(|id| crate::utils::to_user(cache, id)) {
                Some(Some(u)) => format!("{}#{}", u.name, u.discriminator),
                _ => String::new(),
            },
            match MemberRank::decode(&r.get::<String, &str>("rank")) {
                Ok(rank) => rank.to_string(),
                _ => String::new(),
            },
        ]
    })
    .fetch_all(&db.pool)
    .await?)
}

pub async fn list_igns(db: &DB) -> Result<Vec<String>> {
    Ok(sqlx::query!("SELECT ign FROM wynn WHERE mid NOT NULL")
        .map(|r| r.ign)
        .fetch_all(&db.pool)
        .await?)
}

pub async fn stat_leaderboard(
    cache: &Cache, db: &DB, stat: &Stat, filter: &Option<MemberFilter>, no_zero: bool,
) -> Result<(Vec<Vec<String>>, Vec<String>)> {
    let table = stat.table();
    let column = stat.column();
    let link_id = if table == "discord" { "discord" } else { "mcid" };

    let (where_clause, guild_rank_select) = match filter {
        Some(filter) => {
            let clause = some!(filter.where_clause(), bail!("Invalid filter"));
            let select = match filter.guild_rank_select() {
                Some(s) => format!(",{}", s),
                None => String::new(),
            };
            (format!("AND {}", clause), select)
        }
        None => (String::new(), String::new()),
    };

    let mut query = format!(
        "SELECT RANK() OVER(ORDER BY (SELECT {0} FROM {1} WHERE id=member.{2}) DESC) AS r,\
        discord,\
        (SELECT ign FROM wynn WHERE id=member.mcid) AS ign,\
        (SELECT {0} FROM {1} WHERE id=member.{2}) AS stat{3} FROM member \
        WHERE {2} NOT NULL {4}",
        column, table, link_id, guild_rank_select, where_clause
    );
    if no_zero {
        query.push_str(" AND stat");
    }

    let result = sqlx::query(&query)
        .map(|r: SqliteRow| {
            let name = match r.get("ign") {
                Some(ign) => ign,
                None => {
                    match r.get::<Option<i64>, &str>("discord").map(|id| crate::utils::to_user(cache, id)) {
                        Some(Some(u)) => format!("{}#{}", u.name, u.discriminator),
                        _ => String::new(),
                    }
                }
            };
            let lb_rank = r.get::<i64, &str>("r");
            let stat_val = r.get::<i64, &str>("stat");
            vec![lb_rank.to_string(), name, stat.display_stat(stat_val)]
        })
        .fetch_all(&db.pool)
        .await?;
    let header = vec![String::from("#"), String::from("name"), column.to_string()];

    Ok((result, header))
}
