use std::str::FromStr;

use anyhow::{Context as AHContext, Result};
use serenity::client::{Cache, Context};
use serenity::model::guild::{Guild, Member as DMember};
use serenity::model::id::UserId;
use serenity::model::user::User;
use sqlx::sqlite::SqliteRow;

use util::{ioerr, ok, ok_some};

use crate::model::discord::{DiscordId, DiscordProfile};
use crate::model::guild::GuildProfile;
use crate::model::member::{Member, MemberId, MemberRank, MEMBER_RANKS};
use crate::model::wynn::{McId, WynnProfile};
use crate::query::{Column, Filter, MemberName, QueryAction, QueryBuilder, Selectable, Sort};
use crate::DB;

/// Get guild role of a user based on `MemberRank::to_string`
pub async fn get_discord_member_rank(
    ctx: &Context, guild: &Guild, user: &User,
) -> Result<Option<MemberRank>> {
    for rank in MEMBER_RANKS {
        if let Some(role) = rank.get_role(&guild) {
            if user.has_role(&ctx, guild.id, role).await? {
                return Ok(Some(rank));
            }
        }
    }
    Ok(None)
}

/// Remove the role and group role of a member rank from discord member
pub async fn remove_discord_member_rank(
    ctx: &Context, rank: MemberRank, guild: &Guild, member: &mut DMember,
) -> Result<()> {
    let rank_role = rank.get_role(&guild);
    if let Some(role) = rank_role {
        member
            .remove_role(&ctx, role.id)
            .await
            .context("Failed to remove rank role from discord member")?;
    }

    let group_role = rank.get_group_role(&guild);
    if let Some(role) = group_role {
        member
            .remove_role(&ctx, role.id)
            .await
            .context("Failed to remove group role from discord member")?;
    }

    Ok(())
}

/// Remove the rank roles and group roles from discord member except the ones that are associated
/// with the specified member rank
pub async fn remove_discord_member_ranks_except(
    ctx: &Context, rank: MemberRank, guild: &Guild, member: &mut DMember,
) -> Result<()> {
    for other_rank in crate::model::member::MANAGED_MEMBER_RANKS {
        if rank == other_rank {
            continue;
        }

        let rank_role = other_rank.get_role(&guild);
        if let Some(role) = rank_role {
            member
                .remove_role(&ctx, role.id)
                .await
                .context("Failed to remove rank role from discord member")?;
        }

        if !rank.is_same_group(other_rank) {
            let group_role = other_rank.get_group_role(&guild);
            if let Some(role) = group_role {
                member
                    .remove_role(&ctx, role.id)
                    .await
                    .context("Failed to remove group role from discord member")?;
            }
        }
    }

    Ok(())
}

/// Add the role and group role of a member rank to discord member
pub async fn add_discord_member_rank(
    ctx: &Context, rank: MemberRank, guild: &Guild, member: &mut DMember,
) -> Result<()> {
    let rank_role = rank.get_role(&guild);
    if let Some(role) = rank_role {
        member
            .add_role(&ctx, role.id)
            .await
            .context("Failed to add rank role to discord member")?;
    }

    let group_role = rank.get_group_role(&guild);
    if let Some(role) = group_role {
        member
            .add_role(&ctx, role.id)
            .await
            .context("Failed to add group role to discord member")?;
    }

    Ok(())
}

/// Get the discord user from cache with given discord id
pub fn to_user(cache: &Cache, id: DiscordId) -> Option<User> {
    if let Ok(id) = u64::try_from(id.0).map(|id| UserId(id)) {
        return cache.user(id);
    }
    None
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
}

/// Get wynn and guild profile with specified mcid
pub async fn get_wynn_guild_profiles(
    db: &DB, mcid: &Option<McId>,
) -> (Option<WynnProfile>, Option<GuildProfile>) {
    match mcid {
        Some(id) => {
            let wynn = ok!(id.get_wynn(&mut db.exe()).await, None);
            let guild = ok!(id.get_guild(&mut db.exe()).await, None);
            (wynn, guild)
        }
        None => (None, None),
    }
}

/// Get profiles related to the member id
pub async fn get_profiles_member(db: &DB, mid: MemberId) -> Profiles {
    match mid.get(&mut db.exe()).await {
        Ok(Some(member)) => {
            let discord = match member.discord {
                Some(id) => ok!(id.get(&mut db.exe()).await, None),
                None => None,
            };
            let (wynn, guild) = get_wynn_guild_profiles(db, &member.mcid).await;
            Profiles {
                member: Some(member),
                guild,
                discord,
                wynn,
            }
        }
        _ => Profiles {
            member: None,
            guild: None,
            discord: None,
            wynn: None,
        },
    }
}

/// Get profiles related to the discord id
pub async fn get_profiles_discord(db: &DB, discord_id: DiscordId) -> Profiles {
    match discord_id.get(&mut db.exe()).await {
        Ok(Some(discord)) => {
            // Checks if the discord is linked with a member
            if let Some(mid) = discord.mid {
                if let Ok(Some(member)) = mid.get(&mut db.exe()).await {
                    let (wynn, guild) = get_wynn_guild_profiles(db, &member.mcid).await;
                    return Profiles {
                        member: Some(member),
                        guild,
                        discord: Some(discord),
                        wynn,
                    };
                }
            }
            Profiles {
                member: None,
                guild: None,
                discord: Some(discord),
                wynn: None,
            }
        }
        _ => Profiles {
            member: None,
            guild: None,
            discord: None,
            wynn: None,
        },
    }
}

/// Get profiles related to the mcid
pub async fn get_profiles_mc(db: &DB, mcid: &McId) -> Profiles {
    let (wynn, guild) = get_wynn_guild_profiles(db, &Some(mcid.clone())).await;
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
    Profiles { wynn, guild, member, discord }
}

/// Get all ids related to the member
pub async fn get_ids_member(db: &DB, mid: MemberId) -> Ids {
    match mid.exist(&mut db.exe()).await.ok() {
        Some(exist) => {
            if exist {
                let (discord, mc) = ok!(mid.links(&mut db.exe()).await, (None, None));
                Ids { member: Some(mid), discord, mc }
            } else {
                Ids {
                    member: Some(mid),
                    mc: None,
                    discord: None,
                }
            }
        }
        None => Ids { member: None, mc: None, discord: None },
    }
}

/// Checks if the discord user is a member
pub async fn is_discord_member(db: &DB, id: &UserId) -> bool {
    let discord_id = ok!(DiscordId::try_from(id.0), return false);
    match discord_id.mid(&mut db.exe()).await {
        Ok(Some(_)) => true,
        _ => false,
    }
}

#[derive(Debug)]
/// Wrapper over objects that implements `Selectable`
pub enum SelectableWrap {
    Column(Column),
    MemberName(MemberName),
}

impl FromStr for SelectableWrap {
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

impl QueryAction for SelectableWrap {
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        match self {
            Self::Column(col) => col.apply_action(builder),
            Self::MemberName(name) => name.apply_action(builder),
        }
    }
}

impl Selectable for SelectableWrap {
    fn get_formatted(&self, row: &SqliteRow, cache: &Cache) -> String {
        match self {
            Self::Column(col) => col.get_formatted(row, cache),
            Self::MemberName(name) => name.get_formatted(row, cache),
        }
    }
    fn get_table_name(&self) -> &str {
        match self {
            Self::Column(col) => col.get_table_name(),
            Self::MemberName(name) => name.get_table_name(),
        }
    }
}

#[derive(Debug)]
/// Wrapper over `Filter` and `Sort`
pub enum FilterSortWrap {
    Filter(Filter),
    Sort(Sort),
}

impl QueryAction for FilterSortWrap {
    fn apply_action<'a>(&self, builder: &'a mut QueryBuilder) -> &'a mut QueryBuilder {
        match self {
            Self::Filter(filter) => filter.apply_action(builder),
            Self::Sort(sort) => sort.apply_action(builder),
        }
    }
}

pub async fn check_integrity(db: &DB) -> Result<Vec<String>> {
    let mut issues = Vec::new();

    let rows = sqlx::query!(
        "SELECT oid FROM member WHERE 
            (discord NOT NULL AND mcid NOT NULL AND type!='full') OR 
            (discord NOT NULL AND mcid IS NULL AND type!='discord') OR
            (discord IS NULL AND mcid NOT NULL AND 
            NOT (SELECT guild FROM wynn WHERE id=member.mcid) AND type!='wynn') OR 
            (discord IS NULL AND mcid NOT NULL AND 
            (SELECT guild FROM wynn WHERE id=member.mcid) AND type!='guild')"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Wrong member type: {:?}", rows));
    }

    let rows = sqlx::query!("SELECT oid FROM member WHERE (discord IS NULL AND mcid IS NULL)")
        .fetch_all(&db.pool)
        .await?;
    if !rows.is_empty() {
        issues.push(format!("Empty member: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT oid FROM member WHERE 
            (discord NOT NULL AND NOT EXISTS (SELECT 1 FROM discord WHERE id=member.discord AND mid=member.oid)) OR 
            (mcid NOT NULL AND NOT EXISTS (SELECT 1 FROM wynn WHERE id=member.mcid AND mid=member.oid))")
        .fetch_all(&db.pool)
        .await?;
    if !rows.is_empty() {
        issues.push(format!("Bad profile link: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT id FROM discord WHERE
            mid NOT NULL AND NOT EXISTS (SELECT 1 FROM member WHERE oid=discord.mid AND discord=discord.id)"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Bad member link from discord: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT id FROM wynn WHERE 
            mid NOT NULL AND NOT EXISTS (SELECT 1 FROM member WHERE oid=wynn.mid AND mcid=wynn.id)"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Bad member link from wynn: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT id FROM wynn WHERE
            guild AND NOT EXISTS (SELECT 1 FROM guild WHERE id=wynn.id)"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Missing guild profile: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT id FROM guild WHERE
            NOT EXISTS (SELECT 1 FROM wynn WHERE id=guild.id)"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Missing wynn profile: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT id FROM wynn WHERE
            guild AND EXISTS (SELECT 1 FROM guild WHERE id=wynn.id) 
                AND NOT EXISTS (SELECT 1 FROM guild WHERE mid=wynn.mid)"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Mismatched member link between wynn & guild: {:?}", rows));
    }

    Ok(issues)
}
