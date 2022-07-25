use anyhow::{Context as AHContext, Result};
use serenity::client::Context;
use serenity::model::guild::{Guild, Member as DMember};
use serenity::model::id::UserId;
use serenity::model::user::User;

use util::{ok, some2};

use crate::discord::{DiscordId, DiscordProfile};
use crate::guild::GuildProfile;
use crate::member::{Member, MemberId, MemberRank, MEMBER_RANKS};
use crate::wynn::{McId, WynnProfile};
use crate::DB;

/// Get guild role of a user based on `MemberRank::to_string()`
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

pub async fn remove_discord_member_rank(
    ctx: &Context, rank: MemberRank, guild: &Guild, member: &mut DMember,
) -> Result<()> {
    let rank_role = rank.get_role(&guild);
    if let Some(role) = rank_role {
        if member.roles.contains(&role.id) {
            member
                .remove_role(&ctx, role.id)
                .await
                .context("Failed to remove rank role from discord member")?;
        }
    }

    let group_role = rank.get_group_role(&guild);
    if let Some(role) = group_role {
        if member.roles.contains(&role.id) {
            member
                .remove_role(&ctx, role.id)
                .await
                .context("Failed to remove group role from discord member")?;
        }
    }

    Ok(())
}

pub async fn remove_discord_member_ranks_except(
    ctx: &Context, rank: MemberRank, guild: &Guild, member: &mut DMember,
) -> Result<()> {
    for other_rank in crate::member::MANAGED_MEMBER_RANKS {
        if rank == other_rank {
            continue;
        }

        let rank_role = other_rank.get_role(&guild);
        if let Some(role) = rank_role {
            if member.roles.contains(&role.id) {
                member
                    .remove_role(&ctx, role.id)
                    .await
                    .context("Failed to remove rank role from discord member")?;
            }
        }

        if !rank.is_same_group(other_rank) {
            let group_role = other_rank.get_group_role(&guild);
            if let Some(role) = group_role {
                if member.roles.contains(&role.id) {
                    member
                        .remove_role(&ctx, role.id)
                        .await
                        .context("Failed to remove group role from discord member")?;
                }
            }
        }
    }

    Ok(())
}

pub async fn add_discord_member_rank(
    ctx: &Context, rank: MemberRank, guild: &Guild, member: &mut DMember,
) -> Result<()> {
    let rank_role = rank.get_role(&guild);
    if let Some(role) = rank_role {
        if !member.roles.contains(&role.id) {
            member
                .add_role(&ctx, role.id)
                .await
                .context("Failed to add rank role to discord member")?;
        }
    }

    let group_role = rank.get_group_role(&guild);
    if let Some(role) = group_role {
        if !member.roles.contains(&role.id) {
            member
                .add_role(&ctx, role.id)
                .await
                .context("Failed to add group role to discord member")?;
        }
    }

    Ok(())
}

pub fn to_user_id(id: DiscordId) -> Option<UserId> {
    u64::try_from(id).ok().map(|id| UserId(id))
}

pub fn from_user_id(id: UserId) -> Option<DiscordId> {
    i64::try_from(id.0).ok()
}

pub fn to_user(ctx: &Context, id: DiscordId) -> Option<User> {
    if let Some(id) = to_user_id(id) {
        return ctx.cache.user(id);
    }
    None
}

#[derive(Debug)]
pub struct Profiles {
    pub member: Option<Member>,
    pub guild: Option<GuildProfile>,
    pub discord: Option<DiscordProfile>,
    pub wynn: Option<WynnProfile>,
}

impl Profiles {
    pub fn is_none(&self) -> bool {
        self.member.is_none() && self.guild.is_none() && self.discord.is_none() && self.wynn.is_none()
    }
}

#[derive(Debug)]
pub struct Ids {
    pub member: Option<MemberId>,
    pub mc: Option<McId>,
    pub discord: Option<DiscordId>,
}

impl Ids {
    pub async fn to_profiles(&self, db: &DB) -> Profiles {
        let member = match self.member {
            Some(mid) => some2!(crate::get_member(&db, mid).await),
            None => None,
        };
        let guild = match &self.mc {
            Some(mcid) => some2!(crate::get_guild_profile(&db, &mcid).await),
            None => None,
        };
        let wynn = match &self.mc {
            Some(mcid) => some2!(crate::get_wynn_profile(&db, &mcid).await),
            None => None,
        };
        let discord = match self.discord {
            Some(discord) => some2!(crate::get_discord_profile(&db, discord).await),
            None => None,
        };
        Profiles { member, guild, discord, wynn }
    }
}

pub async fn get_wynn_guild_profiles(
    db: &DB, mcid: &Option<McId>,
) -> (Option<WynnProfile>, Option<GuildProfile>) {
    match mcid {
        Some(id) => {
            let wynn = ok!(crate::get_wynn_profile(&db, &id).await, None);
            let guild = ok!(crate::get_guild_profile(&db, &id).await, None);
            (wynn, guild)
        }
        None => (None, None),
    }
}

pub async fn get_profiles_member(db: &DB, mid: MemberId) -> Profiles {
    match crate::get_member(&db, mid).await {
        Ok(Some(member)) => {
            let discord = match member.discord {
                Some(id) => ok!(crate::get_discord_profile(&db, id).await, None),
                None => None,
            };
            let (wynn, guild) = get_wynn_guild_profiles(&db, &member.mcid).await;
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

pub async fn get_profiles_discord(db: &DB, discord_id: DiscordId) -> Profiles {
    match crate::get_discord_profile(&db, discord_id).await {
        Ok(Some(discord)) => {
            if let Some(mid) = discord.mid {
                if let Ok(Some(member)) = crate::get_member(&db, mid).await {
                    let (wynn, guild) = get_wynn_guild_profiles(&db, &member.mcid).await;
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

pub async fn get_profiles_mc(db: &DB, mcid: &McId) -> Profiles {
    let (wynn, guild) = get_wynn_guild_profiles(&db, &Some(mcid.to_string())).await;
    let (member, discord) = match wynn {
        Some(WynnProfile { mid: Some(mid), .. }) => match crate::get_member(&db, mid).await {
            Ok(Some(member)) => match member.discord {
                Some(discord_id) => match crate::get_discord_profile(&db, discord_id).await {
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

pub async fn get_ids_member(db: &DB, mid: MemberId) -> Ids {
    match crate::member_exist(&db, mid).await.ok() {
        Some(exist) => {
            if exist {
                let (discord, mc) = ok!(crate::get_member_links(&db, mid).await, (None, None));
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

pub async fn is_discord_member(db: &DB, id: &UserId) -> bool {
    let discord_id = ok!(i64::try_from(id.0), "Failed to convert u64 to i64 (id)", return false);
    match crate::get_discord_mid(&db, discord_id).await {
        Ok(Some(_)) => true,
        _ => false,
    }
}
