//! Tools for dynamically construct a member table query.
//! Query is built using `QueryBuilder`, by feeding it strings or `QueryAction`.
use std::cmp::Ordering;
use std::collections::HashSet;
use std::str::FromStr;

use anyhow::Result;
use serenity::client::Cache;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use util::ioerr;

use crate::model::db::{Column, ProfileType, Stat};
use crate::model::discord::DiscordId;
use crate::model::guild::{GuildRank, GUILD_RANKS};
use crate::model::member::{MemberRank, MemberType, MEMBER_RANKS};

impl QueryAction for Column {
    /// Selects the column
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        // "`select` AS `identifier`"
        let mut select = self.select_query();
        select.push_str(" AS ");
        select.push_str(self.query_ident());
        builder.select(select)
    }
}

impl Selectable for Column {
    fn format_val(&self, row: &SqliteRow, _: &Cache) -> String {
        let ident = self.query_ident();
        match self {
            // Columns of type String
            Self::MType => row.get(ident),
            // Columns of type Number
            Self::MId => row.get::<i64, _>(ident).to_string(),
            // Columns of type Option<String>
            Self::MDiscord | Self::WIgn | Self::MMcid | Self::GRank => {
                row.get::<Option<String>, _>(ident).unwrap_or_default()
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

    fn table_name(&self) -> &str {
        match self {
            Self::MId => "member_id",
            _ => self.query_ident(),
        }
    }
}

impl SelectAction for Column {
    fn query_ident(&self) -> &str {
        match self {
            Self::GRank => "guild_rank",
            _ => self.name(),
        }
    }

    fn select_query(&self) -> String {
        match self.profile() {
            // If it is from another table
            Some(profile) => {
                format!("(SELECT {} FROM {} WHERE {})", self.name(), profile.name(), profile.select_where(),)
            }
            None => self.name().to_string(),
        }
    }

    fn sort_order(&self) -> Option<&str> {
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

impl QueryAction for Stat {
    /// Selects the stat column
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        self.to_column().apply_action(builder)
    }
}

impl Selectable for Stat {
    fn format_val(&self, row: &SqliteRow, cache: &Cache) -> String {
        self.to_column().format_val(row, cache)
    }

    fn table_name(&self) -> &str {
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
            Self::InGuild => builder.with(&Column::WGuild).filter(Column::WGuild.query_ident().to_string()),
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
                    Column::GRank.query_ident(),
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
                builder.with(&col).filter(format!("{}{}{}", col.query_ident(), cmp, val))
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

            if let Ok(member_type) = MemberType::from_str(s) {
                return Ok(Self::MemberType(member_type));
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
        ioerr!("Failed to parse '{}' as Filter", s)
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

        match sa.sort_order() {
            Some(case) => {
                builder.order(format!("CASE {} {} END {} NULLS LAST", sa.select_query(), case, order))
            }
            None => builder.order(format!("{} {} NULLS LAST", sa.select_query(), order)),
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
            if let Some(stripped) = s.strip_prefix('^') {
                if let Ok(col) = Column::from_str(stripped) {
                    return Ok(Self::Asc(col));
                }
            } else if let Ok(col) = Column::from_str(s) {
                return Ok(Self::Desc(col));
            }
        }

        ioerr!("Failed to parse '{}' as Sort", s)
    }
}

#[derive(Debug)]
/// Wrapper over `Filter` and `Sort`
pub enum QueryMod {
    Filter(Filter),
    Sort(Sort),
}

impl QueryAction for QueryMod {
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        match self {
            Self::Filter(filter) => filter.apply_action(builder),
            Self::Sort(sort) => sort.apply_action(builder),
        }
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
    fn format_val(&self, row: &SqliteRow, cache: &Cache) -> String {
        match row.get(Column::WIgn.query_ident()) {
            Some(ign) => ign,
            None => match row.get::<Option<DiscordId>, &str>("discord").map(|id| id.to_user(cache)) {
                Some(Some(u)) => format!("{}#{}", u.name, u.discriminator),
                _ => String::new(),
            },
        }
    }

    fn table_name(&self) -> &str {
        "name"
    }
}

impl FromStr for MemberName {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "name" {
            Ok(MemberName)
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
    fn format_val(&self, _: &SqliteRow, _: &Cache) -> String;
    /// Get the column name to be displayed in a table
    fn table_name(&self) -> &str;
}

#[derive(Debug)]
/// Wrapper over objects that implements `Selectable`
pub enum Selectables {
    Column(Column),
    MemberName(MemberName),
}

impl FromStr for Selectables {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(col) = Column::from_str(s) {
            return Ok(Self::Column(col));
        }
        if let Ok(name) = MemberName::from_str(s) {
            return Ok(Self::MemberName(name));
        }
        ioerr!("Failed to parse '{}' as Selectables", s)
    }
}

impl QueryAction for Selectables {
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        match self {
            Self::Column(col) => col.apply_action(builder),
            Self::MemberName(name) => name.apply_action(builder),
        }
    }
}

impl Selectable for Selectables {
    fn format_val(&self, row: &SqliteRow, cache: &Cache) -> String {
        match self {
            Self::Column(col) => col.format_val(row, cache),
            Self::MemberName(name) => name.format_val(row, cache),
        }
    }

    fn table_name(&self) -> &str {
        match self {
            Self::Column(col) => col.table_name(),
            Self::MemberName(name) => name.table_name(),
        }
    }
}

/// `QueryAction` that performs a column select
pub trait SelectAction: Selectable {
    /// Get the identifier of the selected value
    fn query_ident(&self) -> &str;
    /// Get the select statement (without the identifier)
    fn select_query(&self) -> String;
    /// Get the selected value's custom sort order, if it needs one
    fn sort_order(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Clone)]
/// Dynamic builder for query string
pub struct QueryBuilder {
    select_tokens: HashSet<String>,
    where_tokens: HashSet<String>,
    order_tokens: Vec<String>,
}

impl QueryBuilder {
    pub fn new() -> Self {
        Self {
            select_tokens: HashSet::new(),
            where_tokens: HashSet::new(),
            order_tokens: Vec::new(),
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
        if !self.order_tokens.contains(&token) {
            self.order_tokens.push(token);
        }
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

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}
