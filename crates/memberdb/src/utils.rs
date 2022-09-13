use anyhow::{Context as AHContext, Result};
use serenity::client::Context;
use serenity::model::guild::{Guild, Member as DMember};
use serenity::model::id::UserId;
use serenity::model::user::User;

use util::ok;

use crate::model::discord::DiscordId;
use crate::model::guild::GuildProfile;
use crate::model::member::{MemberRank, MEMBER_RANKS};
use crate::model::wynn::{McId, WynnProfile};
use crate::DB;

/// Get guild role of a user based on `MemberRank::to_string`
pub async fn get_discord_member_rank(
    ctx: &Context, guild: &Guild, user: &User,
) -> Result<Option<MemberRank>> {
    for rank in MEMBER_RANKS {
        if let Some(role) = rank.get_role(guild) {
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
    let rank_role = rank.get_role(guild);
    if let Some(role) = rank_role {
        member
            .remove_role(&ctx, role.id)
            .await
            .context("Failed to remove rank role from discord member")?;
    }

    let group_role = rank.get_group_role(guild);
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

        let rank_role = other_rank.get_role(guild);
        if let Some(role) = rank_role {
            member
                .remove_role(&ctx, role.id)
                .await
                .context("Failed to remove rank role from discord member")?;
        }

        if !rank.is_same_group(other_rank) {
            let group_role = other_rank.get_group_role(guild);
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
    let rank_role = rank.get_role(guild);
    if let Some(role) = rank_role {
        member
            .add_role(&ctx, role.id)
            .await
            .context("Failed to add rank role to discord member")?;
    }

    let group_role = rank.get_group_role(guild);
    if let Some(role) = group_role {
        member
            .add_role(&ctx, role.id)
            .await
            .context("Failed to add group role to discord member")?;
    }

    Ok(())
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

/// Checks if the discord user is a member
pub async fn is_discord_member(db: &DB, id: &UserId) -> bool {
    let discord_id = ok!(DiscordId::try_from(id.0), return false);
    matches!(discord_id.mid(&mut db.exe()).await, Ok(Some(_)))
}
