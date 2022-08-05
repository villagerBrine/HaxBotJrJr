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
    /// Return which profile table a column belongs to, None if it is part of the member table
    pub fn profile(&self) -> Option<ProfileType> {
        match self {
            Self::MId | Self::MRank | Self::MType | Self::MMcid | Self::MDiscord => None,
            Self::GRank | Self::GXp | Self::GWeeklyXp => Some(ProfileType::Guild),
            Self::WGuild | Self::WIgn | Self::WOnline | Self::WWeeklyOnline => Some(ProfileType::Wynn),
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
    /// Selects the column
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        // "`select` AS `identifier`"
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
            // Columns of type String
            Self::MType => row.get(ident),
            // Columns of type Number
            Self::MId => row.get::<i64, _>(ident).to_string(),
            // Columns of type Option<String>
            Self::MDiscord | Self::WIgn | Self::MMcid | Self::GRank => {
                row.get::<Option<String>, _>(ident).unwrap_or(String::new())
            }
            // Columns of type Option<Number>
            Self::DMessage | Self::DWeeklyMessage | Self::GXp | Self::GWeeklyXp => {
                match row.get::<Option<i64>, _>(ident) {
                    Some(n) => util::string::fmt_num(n, true),
                    None => String::new(),
                }
            }
            // Columns of type Option<Time Duration>
            Self::DVoice | Self::DWeeklyVoice | Self::WOnline | Self::WWeeklyOnline => {
                match row.get::<Option<i64>, _>(ident) {
                    Some(n) => util::string::fmt_second(n),
                    None => String::new(),
                }
            }
            // Columns of type Option<Boolean>
            Self::WGuild => match row.get::<Option<i64>, _>(ident) {
                Some(1) => "true".to_string(),
                _ => "false".to_string(),
            },
            // Columns of type MemberRank
            Self::MRank => {
                let s = row.get::<&str, _>(ident);
                match MemberRank::decode(s) {
                    Ok(r) => r.to_string(),
                    Err(_) => String::new(),
                }
            }
        }
    }

    fn get_table_name(&self) -> &str {
        match self {
            Self::MId => "member_id",
            _ => self.get_ident(),
        }
    }
}

impl SelectAction for Column {
    fn get_ident(&self) -> &str {
        match self {
            Self::GRank => "guild_rank",
            _ => self.name(),
        }
    }

    fn get_select(&self) -> String {
        match self.profile() {
            // If it is from another table
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
                WHEN 'Two' THEN 4
                WHEN 'One' THEN 5
                WHEN 'Zero' THEN 6"
            }
            _ => return None,
        })
    }
}

impl ProfileType {
    /// Get the name of the corresponding database table
    pub fn name(&self) -> &str {
        match self {
            Self::Guild => "guild",
            Self::Wynn => "wynn",
            Self::Discord => "discord",
        }
    }

    /// The condition needed to located the profile from member table
    pub fn select_where(&self) -> &str {
        match self {
            Self::Guild => "id=member.mcid",
            Self::Wynn => "mid=member.oid",
            Self::Discord => "id=member.discord",
        }
    }
}

impl QueryAction for ProfileType {
    /// Filter out members without the profile
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        match self {
            Self::Wynn => builder.filter("mcid NOT NULL".to_string()),
            Self::Guild => builder.filter("(SELECT guild FROM wynn WHERE mid=member.oid)".to_string()),
            Self::Discord => builder.filter("discord NOT NULL".to_string()),
        }
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
            Self::Xp => Column::GXp,
            Self::WeeklyXp => Column::GWeeklyXp,
        }
    }

    /// Parse string into stat value based on stat type
    pub fn parse_val(&self, val: &str) -> Result<u64> {
        match self {
            // parse as time duration
            Self::Voice | Self::WeeklyVoice | Self::Online | Self::WeeklyOnline => {
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

impl QueryAction for Stat {
    /// Selects the stat column
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        self.to_column().apply_action(builder)
    }
}

impl Selectable for Stat {
    fn get_formatted(&self, row: &SqliteRow, cache: &Cache) -> String {
        self.to_column().get_formatted(row, cache)
    }

    fn get_table_name(&self) -> &str {
        match self {
            Self::Message => "message",
            Self::WeeklyMessage => "weekly_message",
            Self::Voice => "voice",
            Self::WeeklyVoice => "weekly_voice",
            Self::Online => "online",
            Self::WeeklyOnline => "weekly_online",
            Self::Xp => "xp",
            Self::WeeklyXp => "weekly_xp",
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
/// Member filters
pub enum Filter {
    /// Filter out full members
    Partial,
    /// Filter out members who aren't in the in-game guild
    InGuild,
    /// Filter out members without linked mc account
    HasMc,
    /// Filter out members without linked discord account
    HasDiscord,
    /// Filter out members that isn't the specified member type
    MemberType(MemberType),
    /// Filter out members by member rank.
    MemberRank(MemberRank, Ordering),
    /// Filter out members by guild rank.
    GuildRank(GuildRank, Ordering),
    /// Filter out members by stat value.
    Stat(Stat, u64, Ordering),
}

impl QueryAction for Filter {
    /// Apply the filter
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

    /// Given a filter named "filter", it can take the form of:
    /// - "filter"
    /// - ">filter", "<filter" if it supports ordered filter
    /// - "filter:val", ">filter:val", "<filter:val" if it is a stat filter
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
/// Represent sorting of a column
pub enum Sort {
    Asc(Column),
    Desc(Column),
}

impl QueryAction for Sort {
    /// Apply the sorting
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

impl FromStr for Sort {
    type Err = std::io::Error;

    /// Possible formats:
    /// "column" - order column in descend order
    /// "^column" - order column in ascend order
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_empty() {
            if s.starts_with('^') {
                if let Ok(col) = Column::from_str(&s[1..]) {
                    return Ok(Self::Asc(col));
                }
            } else {
                if let Ok(col) = Column::from_str(s) {
                    return Ok(Self::Desc(col));
                }
            }
        }

        ioerr!("Failed to parse '{}' as Sort", s)
    }
}

#[derive(Debug)]
/// Implements `Selectable` that gives you the name of a member.
/// The name if the member's ign if it exists, otherwise it is their discord username.
pub struct MemberName;

impl QueryAction for MemberName {
    /// Selects ign and discord id, needed for making the member's name
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        builder.with(&Column::WIgn).with(&Column::MDiscord)
    }
}

impl Selectable for MemberName {
    /// Get the name of the member
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

    fn get_table_name(&self) -> &str {
        "name"
    }
}

impl FromStr for MemberName {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "name" {
            return Ok(MemberName);
        } else {
            ioerr!("Failed to parse '{}' as MemberName", s)
        }
    }
}

/// Trait for object that can modify `QueryBuilder`, used through `QueryBuilder.with`
pub trait QueryAction {
    /// Modify `QueryAction`
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder;
}

/// Trait for extracting value from `SqliteRow`, helps with table display
pub trait Selectable: QueryAction + Sync {
    /// Extract value from `SqliteRow` as formatted string
    fn get_formatted(&self, _: &SqliteRow, _: &Cache) -> String;
    /// Get the column name to be displayed in a table
    fn get_table_name(&self) -> &str;
}

/// `QueryAction` that performs a column select
pub trait SelectAction: Selectable {
    /// Get the identifier of the selected value
    fn get_ident(&self) -> &str;
    /// Get the select statement (without the identifier)
    fn get_select(&self) -> String;
    /// Get the selected value's custom sort order, if it needs one
    fn get_sort_order(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Clone)]
/// Dynamic builder for query string
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

    /// Add a "select" expression
    pub fn select(&mut self, token: String) -> &mut Self {
        self.select_tokens.insert(token);
        self
    }

    /// Add a "where" expression
    pub fn filter(&mut self, token: String) -> &mut Self {
        self.where_tokens.insert(token);
        self
    }

    /// Add a "order by" expression
    pub fn order(&mut self, token: String) -> &mut Self {
        self.order_tokens.insert(token);
        self
    }

    /// Apply action
    pub fn with(&mut self, action: &impl QueryAction) -> &mut Self {
        action.apply_action(self)
    }

    /// Build query string
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

    /// Build leaderboard query string, with ranking number.
    /// `rank_name` is the identifier of the rank number.
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
        } else {
            query.push_str(", RANK() OVER(ORDER BY oid) AS ");
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
