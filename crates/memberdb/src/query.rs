//! Tools for dynamically construct a member table query.
//! Query is built using `QueryBuilder`, by feeding it strings or `QueryAction`.
use std::cmp::Ordering;
use std::collections::HashSet;
use std::str::FromStr;

use anyhow::{bail, Result};
use serenity::client::Cache;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use util::ioerr;

use crate::model::guild::{GuildRank, GUILD_RANKS};
use crate::model::member::{MemberRank, MemberType, ProfileType, MEMBER_RANKS};

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
    pub fn profile(&self) -> Option<ProfileType> {
        match self {
            Self::MId | Self::MRank | Self::MType | Self::MMcid | Self::MDiscord => None,
            Self::GRank | Self::GXp | Self::GWeeklyXp => Some(ProfileType::Guild),
            Self::WGuild | Self::WIgn | Self::WOnline | Self::WWeeklyOnline => Some(ProfileType::Wynn),
            _ => Some(ProfileType::Discord),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::MDiscord => "discord",
            Self::MMcid => "mcid",
            Self::MId => "oid",
            Self::DMessage => "message",
            Self::DWeeklyMessage => "message_week",
            Self::DVoice => "voice",
            Self::DWeeklyVoice => "voice_week",
            Self::WGuild => "in_guild",
            Self::WIgn => "ign",
            Self::WOnline => "activity",
            Self::WWeeklyOnline => "activity_week",
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

impl QueryAction for Column {
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        let mut select = self.get_select();
        select.push_str(" AS ");
        select.push_str(self.get_ident());
        builder.select(select)
    }
}

impl Selectable for Column {
    fn get_formatted(&self, row: &SqliteRow, _: &Cache) -> String {
        let ident = self.get_ident();
        match self {
            Self::MId | Self::MType => row.get(ident),
            Self::MDiscord | Self::WIgn | Self::MMcid | Self::GRank => {
                row.get::<Option<String>, _>(ident).unwrap_or(String::new())
            }
            Self::DMessage | Self::DWeeklyMessage | Self::GXp | Self::GWeeklyXp => {
                match row.get::<Option<i64>, _>(ident) {
                    Some(n) => util::string::fmt_num(n, true),
                    None => String::new(),
                }
            }
            Self::DVoice | Self::DWeeklyVoice | Self::WOnline | Self::WWeeklyOnline => {
                match row.get::<Option<i64>, _>(ident) {
                    Some(n) => util::string::fmt_second(n),
                    None => String::new(),
                }
            }
            Self::WGuild => match row.get::<Option<i64>, _>(ident) {
                Some(1) => "true".to_string(),
                _ => "false".to_string(),
            },
            Self::MRank => {
                let s = row.get::<&str, _>(ident);
                match MemberRank::decode(s) {
                    Ok(r) => r.to_string(),
                    Err(_) => String::new(),
                }
            }
        }
    }
}

impl SelectAction for Column {
    fn get_table_name(&self) -> &str {
        match self {
            Self::MId => "member_id",
            _ => self.get_ident(),
        }
    }

    fn get_ident(&self) -> &str {
        match self {
            Self::GRank => "guild_rank",
            _ => self.name(),
        }
    }

    fn get_select(&self) -> String {
        match self.profile() {
            Some(profile) => {
                format!("(SELECT {} FROM {} WHERE {})", self.name(), profile.name(), profile.select_where(),)
            }
            None => self.name().to_string(),
        }
    }

    fn get_sort_order(&self) -> Option<&str> {
        Some(match self {
            Self::GRank => {
                "WHEN 'Recruit' THEN 0 
                WHEN 'Recruiter' THEN 1 
                WHEN 'Captain' THEN 2
                WHEN 'Strategist' THEN 3
                WHEN 'Chief' THEN 4
                WHEN 'Owner' THEN 5"
            }
            Self::MRank => {
                "WHEN 'Six' THEN 0
                WHEN 'Five' THEN 1
                WHEN 'Four' THEN 2
                WHEN 'Three' THEN 3
                WHEN 'Two' THEN 4,
                WHEN 'One' THEN 5,
                WHEN 'Zero' THEN 6"
            }
            _ => return None,
        })
    }
}

impl ProfileType {
    pub fn name(&self) -> &str {
        match self {
            Self::Guild => "guild",
            Self::Wynn => "wynn",
            Self::Discord => "discord",
        }
    }

    pub fn select_where(&self) -> &str {
        match self {
            Self::Guild => "id=member.mcid",
            Self::Wynn => "mid=member.oid",
            Self::Discord => "id=member.discord",
        }
    }

    pub fn exist_where(&self) -> &str {
        match self {
            Self::Guild => "(SELECT guild FROM wynn WHERE mid=member.oid)",
            Self::Wynn => "mcid",
            Self::Discord => "discord",
        }
    }
}

impl QueryAction for ProfileType {
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        match self {
            Self::Guild | Self::Wynn => builder.filter("mcid NOT NULL".to_string()),
            Self::Discord => builder.filter("discord NOT NULL".to_string()),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Stat {
    Message,
    WeeklyMessage,
    Voice,
    WeeklyVoice,
    Online,
    WeeklyOnline,
    Xp,
    WeeklyXp,
}

impl Stat {
    pub fn from_column(col: &Column) -> Option<Self> {
        Some(match col {
            Column::DMessage => Self::Message,
            Column::DWeeklyMessage => Self::WeeklyMessage,
            Column::DVoice => Self::Voice,
            Column::DWeeklyVoice => Self::WeeklyVoice,
            Column::WOnline => Self::Online,
            Column::WWeeklyOnline => Self::WeeklyOnline,
            Column::GXp => Self::Xp,
            Column::GWeeklyXp => Self::WeeklyXp,
            _ => return None,
        })
    }

    pub fn to_column(&self) -> Column {
        match self {
            Self::Message => Column::DMessage,
            Self::WeeklyMessage => Column::DWeeklyMessage,
            Self::Voice => Column::DVoice,
            Self::WeeklyVoice => Column::DWeeklyVoice,
            Self::Online => Column::WOnline,
            Self::WeeklyOnline => Column::WWeeklyOnline,
            Self::Xp => Column::GXp,
            Self::WeeklyXp => Column::GWeeklyXp,
        }
    }

    pub fn parse_val(&self, val: &str) -> Result<u64> {
        match self {
            Self::Voice | Self::WeeklyVoice | Self::Online | Self::WeeklyOnline => {
                util::string::parse_second(val)
            }
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

impl QueryAction for Stat {
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        self.to_column().apply_action(builder)
    }
}

impl Selectable for Stat {
    fn get_formatted(&self, row: &SqliteRow, cache: &Cache) -> String {
        self.to_column().get_formatted(row, cache)
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Filter {
    Partial,
    InGuild,
    HasMc,
    HasDiscord,
    MemberType(MemberType),
    MemberRank(MemberRank, Ordering),
    GuildRank(GuildRank, Ordering),
    Stat(Stat, u64, Ordering),
}

impl QueryAction for Filter {
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        match self {
            Self::Partial => builder.filter("type!='full'".to_string()),
            Self::InGuild => builder.with(&Column::WGuild).filter(Column::WGuild.get_ident().to_string()),
            Self::HasMc => builder.filter("mcid NOT NULL".to_string()),
            Self::HasDiscord => builder.filter("discord NOT NULL".to_string()),
            Self::MemberType(ty) => builder.filter(format!("type='{}'", ty)),
            Self::MemberRank(target_rank, target_ord) => {
                let mut valid_ranks = Vec::new();
                for rank in MEMBER_RANKS {
                    let ord = rank.cmp(target_rank);
                    if ord == Ordering::Equal || ord == *target_ord {
                        valid_ranks.push(format!("'{:?}'", rank));
                    }
                }
                let valid_ranks = valid_ranks.join(",");
                builder.filter(format!("rank IN ({})", valid_ranks))
            }
            Self::GuildRank(target_rank, target_ord) => {
                let mut valid_ranks = Vec::new();
                for rank in GUILD_RANKS {
                    let ord = rank.cmp(target_rank);
                    if ord == Ordering::Equal || ord == *target_ord {
                        valid_ranks.push(format!("'{}'", rank));
                    }
                }
                let valid_ranks = valid_ranks.join(",");
                builder.with(&Column::GRank).filter(format!(
                    "{} IN ({})",
                    Column::GRank.get_ident(),
                    valid_ranks
                ))
            }
            Self::Stat(stat, val, ord) => {
                let cmp = match ord {
                    Ordering::Equal => "=",
                    Ordering::Less => "<=",
                    Ordering::Greater => ">=",
                };
                let col = stat.to_column();
                builder.with(&col).filter(format!("{}{}{}", col.get_ident(), cmp, val))
            }
        }
    }
}

impl FromStr for Filter {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_empty() {
            match s {
                "partial" => return Ok(Self::Partial),
                "in_guild" => return Ok(Self::InGuild),
                "has_mc" => return Ok(Self::HasMc),
                "has_discord" => return Ok(Self::HasDiscord),
                _ => {}
            }

            if let Ok(stat) = Stat::from_str(s) {
                return Ok(Self::Stat(stat, 1, Ordering::Greater));
            }

            let (ord, s) = {
                let symbol = s.chars().next().unwrap();
                match symbol {
                    '<' => (Ordering::Less, &s[1..]),
                    '>' => (Ordering::Greater, &s[1..]),
                    _ => (Ordering::Equal, s),
                }
            };

            if s.contains(':') {
                if let Some((stat_name, val)) = s.split_once(':') {
                    if let Ok(stat) = Stat::from_str(stat_name) {
                        if let Ok(val) = stat.parse_val(val) {
                            return Ok(Self::Stat(stat, val, ord));
                        }
                    }
                }
            } else {
                if let Ok(rank) = MemberRank::from_str(s) {
                    return Ok(Self::MemberRank(rank, ord));
                }
                if let Ok(rank) = GuildRank::from_str(s) {
                    return Ok(Self::GuildRank(rank, ord));
                }
            }
        }
        return ioerr!("Failed to parse '{}' as Filter", s);
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Sort<S: SelectAction> {
    Asc(S),
    Desc(S),
}

impl Sort<Column> {
    pub fn new(col: Column, asc: bool) -> Sort<Column> {
        if asc {
            Self::Asc(col)
        } else {
            Self::Desc(col)
        }
    }
}

impl<T: SelectAction> QueryAction for Sort<T> {
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        let (sa, order) = match self {
            Self::Asc(sa) => (sa, "ASC"),
            Self::Desc(sa) => (sa, "DESC"),
        };

        match sa.get_sort_order() {
            Some(case) => {
                builder.order(format!("CASE {} {} END {} NULLS LAST", sa.get_select(), case, order))
            }
            None => builder.order(format!("{} {} NULLS LAST", sa.get_select(), order)),
        }
    }
}

#[derive(Debug)]
pub struct MemberName;

impl QueryAction for MemberName {
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        builder.with(&Column::WIgn).with(&Column::MDiscord)
    }
}

impl Selectable for MemberName {
    fn get_formatted(&self, row: &SqliteRow, cache: &Cache) -> String {
        match row.get(Column::WIgn.get_ident()) {
            Some(ign) => ign,
            None => match row
                .get::<Option<i64>, &str>("discord")
                .map(|id| crate::utils::to_user(cache, id))
            {
                Some(Some(u)) => format!("{}#{}", u.name, u.discriminator),
                _ => String::new(),
            },
        }
    }
}

pub trait QueryAction {
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder;
}

pub trait Selectable: QueryAction {
    fn get_formatted(&self, _: &SqliteRow, _: &Cache) -> String;
}

pub trait SelectAction: Selectable {
    fn get_table_name(&self) -> &str;
    fn get_ident(&self) -> &str;
    fn get_select(&self) -> String;
    fn get_sort_order(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct QueryBuilder {
    select_tokens: HashSet<String>,
    where_tokens: HashSet<String>,
    order_tokens: HashSet<String>,
}

impl QueryBuilder {
    pub fn new() -> Self {
        Self {
            select_tokens: HashSet::new(),
            where_tokens: HashSet::new(),
            order_tokens: HashSet::new(),
        }
    }

    pub fn select(&mut self, token: String) -> &mut Self {
        self.select_tokens.insert(token);
        self
    }

    pub fn filter(&mut self, token: String) -> &mut Self {
        self.where_tokens.insert(token);
        self
    }

    pub fn order(&mut self, token: String) -> &mut Self {
        self.order_tokens.insert(token);
        self
    }

    pub fn with(&mut self, action: &impl QueryAction) -> &mut Self {
        action.apply_action(self)
    }

    pub fn build(self) -> String {
        let mut query = if !self.select_tokens.is_empty() {
            let select = self.select_tokens.into_iter().collect::<Vec<String>>().join(",");
            format!("SELECT {} FROM member", select)
        } else {
            String::from("SELECT oid FROM MEMBER")
        };

        if !self.where_tokens.is_empty() {
            let filter = self.where_tokens.into_iter().collect::<Vec<String>>().join(" AND ");
            query.push_str(" WHERE ");
            query.push_str(&filter);
        }

        if !self.order_tokens.is_empty() {
            let order = self.order_tokens.into_iter().collect::<Vec<String>>().join(",");
            query.push_str(" ORDER BY ");
            query.push_str(&order);
        }

        query
    }

    pub fn build_lb(self, rank_name: &str) -> String {
        let mut query = if !self.select_tokens.is_empty() {
            let select = self.select_tokens.into_iter().collect::<Vec<String>>().join(",");
            format!("SELECT {}", select)
        } else {
            String::from("SELECT oid")
        };

        if !self.order_tokens.is_empty() {
            let order = self.order_tokens.into_iter().collect::<Vec<String>>().join(",");
            query.push_str(", RANK() OVER(ORDER BY ");
            query.push_str(&order);
            query.push_str(") AS ");
            query.push_str(rank_name);
        }

        query.push_str(" FROM member ");

        if !self.where_tokens.is_empty() {
            let filter = self.where_tokens.into_iter().collect::<Vec<String>>().join(" AND ");
            query.push_str(" WHERE ");
            query.push_str(&filter);
        }

        query
    }
}
