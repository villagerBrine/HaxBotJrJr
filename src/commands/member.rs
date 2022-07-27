use std::str::FromStr;

use anyhow::Context as AHContext;
use serenity::client::Context;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::Message;
use tokio::sync::RwLock;
use tracing::error;

use memberdb::error::DBError;
use memberdb::events::DBEvent;
use memberdb::member::{MemberId, MemberRank, ProfileType, MemberType};
use memberdb::table::{MemberFilter, Stat};
use memberdb::DB;
use msgtool::pager::Pager;
use msgtool::parser::{DiscordObject, TargetObject};
use msgtool::table::TableData;
use util::{ok, some, ctx};

use crate::checks::{MAINSERVER_CHECK, STAFF_CHECK};
use crate::util::db::TargetId;
use crate::{arg, cmd_bail, data, finish, send, send_embed};

#[command("profile")]
#[only_in(guild)]
#[usage("<target>")]
#[example("m:Pucaet")]
#[example("d:Pucaet")]
#[example("d:Pucaet#9528")]
/// Display the profile of `target`.
/// Contents of other profiles are also included if they are linked with `target`.
///
/// > **How do I specify different targets**
/// - __Discord user__: "d:<username>", ex: "d:Pucaet" or "d:Pucaet#9528"
/// - __Mc account__: "m:<ign>", ex: "m:SephDark18"
async fn display_profile(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get the associated guild"));
    let (db, client) = data!(ctx, "db", "reqwest");

    let target = {
        let arg = args.rest();
        if arg.is_empty() {
            TargetId::Discord(msg.author.id)
        } else {
            crate::parse_user_target!(ctx, msg, db, client, guild, args.rest())
        }
    };
    let profiles = {
        let db = db.read().await;
        match target {
            TargetId::Discord(id) => {
                let id = ctx!(
                    i64::try_from(id.0),
                    "Failed to convert u64 into i64"
                )?;
                memberdb::utils::get_profiles_discord(&db, id).await
            }
            TargetId::Wynn(id) => {memberdb::utils::get_profiles_mc(&db, &id).await
            }
        }
    };

    if profiles.is_none() {
        finish!(ctx, msg, "No profiles found");
    }

    let names = msgtool::profile::get_names(ctx, &profiles).await;

    send_embed!(ctx, msg, |e| {
        e.author(|a| a.name(names.0)).title(names.1);

        if let Some(discord) = &profiles.discord {
            if let Some(user) = memberdb::utils::to_user(&ctx, discord.id) {
                if let Some(url) = user.avatar_url() {
                    e.thumbnail(url);
                }
            }
        }

        for (name, value) in msgtool::profile::get_guild_stat_fields(&profiles.guild) {
            e.field(name, value, true);
        }
        for (name, value) in msgtool::profile::get_wynn_stat_fields(&profiles.wynn) {
            e.field(name, value, true);
        }
        for (name, value) in msgtool::profile::get_discord_stat_fields(&profiles.discord) {
            e.field(name, value, true);
        }

        if profiles.member.is_none() {
            e.footer(|f| f.text("This is an unlinked profile"));
        }

        e
    });

    Ok(())
}

#[command("addMember")]
#[only_in(guild)]
#[checks(MainServer, Staff)]
#[usage("<discord_user> <ign>")]
#[example("Pucaet#9528 Pucaet\n")]
/// Add a new member with provided discord and mc accounts.
/// `discord_user` is a discord username, ex: `Pucaet` or `Pucaet#9528`.
///
/// > **How the initial rank is determined**
/// if `ign` is in guild, their guild rank is used,
/// otherwise the bot attempts to find a rank role on `discord_user` and use that.
/// If all fails, the lowest rank is used.
pub async fn add_member(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let (discord_name, ign) = arg!(ctx, msg, args, 
        String: "Discord user name not provided",
        String: "Mc ign not provided");

    let (db, client) = data!(ctx, "db", "reqwest");
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));

    let (discord_member, discord_id, mcid) =
        crate::get_profile_ids!(ctx, msg, guild, client, discord_name, ign);

    // Check for precondition. Both profiles has to be unlinked
    let (wynn_mid, discord_mid) = crate::util::db::get_profile_mids(&db, discord_id, &mcid).await;
    if discord_mid.is_some() && wynn_mid.is_some() && discord_mid == wynn_mid {
        finish!(ctx, msg, "Both profiles are already linked to the same member");
    }
    if wynn_mid.is_some() || discord_mid.is_some() {
        finish!(ctx, msg, "At least one of the provided profiles is already linked to a member. If you want to update / add \
profiles on an existing member, use the command `link` instead");
    }

    // Getting initial member rank
    let guild_rank = {
        let db = db.read().await;
        memberdb::get_guild_rank(&db, &mcid).await
    };
    let rank = match guild_rank {
        Ok(guild_rank) => guild_rank.to_member_rank(),
        Err(_) => {
            match memberdb::utils::get_discord_member_rank(&ctx, &guild, &discord_member.as_ref().user).await
            {
                Ok(Some(rank)) => rank,
                _ => memberdb::member::INIT_MEMBER_RANK,
            }
        }
    };

    let result = {
        let db = db.write().await;
        ctx!(memberdb::add_member(&db, discord_id, &mcid, &ign, rank).await, "Failed to add member")
    };

    finish!(
        ctx,
        msg,
        match result {
            Ok(_) => "Successfully added member",
            Err(_) => "Failed to add member",
        }
    )
}

#[command("link")]
#[only_in(guild)]
#[checks(MainServer, Staff)]
#[usage("<discord_user> <ign>")]
#[example("Pucaet#9528 Pucaet")]
/// Link discord and mc accounts to the same member.
/// `discord_user` is a discord username, ex: `Pucaet` or `Pucaet#9528`.
///
/// This command only accepts one linked account representing an existing member,
/// and an unlinked account to be linked to that member.
///
/// This can be used to add an account to member, or update one's account.
/// Note that you can't update a member's mc account.
pub async fn link_profile(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let (discord_name, ign) = arg!(ctx, msg, args, 
        String: "Discord user name not provided", 
        String: "Mc ign not provided");

    let (db, client) = data!(ctx, "db", "reqwest");
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));

    let (_discord_member, discord_id, mcid) =
        crate::get_profile_ids!(ctx, msg, guild, client, discord_name, ign);

    let (wynn_mid, discord_mid) = crate::util::db::get_profile_mids(&db, discord_id, &mcid).await;
    if discord_mid.and(wynn_mid).is_some() && discord_mid == wynn_mid {
        finish!(ctx, msg, "Both profiles are already linked to the same member");
    }
    if discord_mid.or(wynn_mid).is_none() {
        finish!(ctx, msg, "Both profiles are unlinked. If you want to add a new member, use the command `addMember` instead");
    }
    if discord_mid.and(wynn_mid).is_some() {
        finish!(
            ctx,
            msg,
            "Both profiles are linked. If you want to link both profiles to the same member, \
unlink one of them first, then call this command again"
        )
    }

    // Updating discord profile link
    if let Some(mid) = wynn_mid {
        let result = {
            let db = db.write().await;
            ctx!(
                memberdb::bind_discord(&db, mid, Some(discord_id)).await,
                "Failed to link discord profile to member"
            )
        };

        finish!(
            ctx,
            msg,
            match result {
                Ok(_) => "Successfully linked discord user to member",
                Err(_) => "Failed to link discord user to member",
            }
        )
    // Updating wynn profile linke
    } else if let Some(mid) = discord_mid {
        {
            // Checking if member already have a wynn profile
            let db = db.read().await;
            if let Some(ign) = existing_wynn_link_check(&db, mid).await {
                finish!(ctx, msg,
                    format!("The discord user `{}` already has a mc account `{}` linked to them, which can't be changed",
                            discord_name, ign));
            }
        }

        let result = {
            let db = db.write().await;
            ctx!(
                memberdb::bind_wynn(&db, mid, Some(&mcid), &ign).await,
                "Failed to link wynn profile to member"
            )
        };

        finish!(
            ctx,
            msg,
            match result {
                Ok(_) => "Successfully linked mc user to member",
                Err(_) => "Failed to link mc user to member",
            }
        )
    }

    Ok(())
}

#[command("addPartial")]
#[only_in(guild)]
#[checks(MainServer, Staff)]
#[usage("<discord | wynn> <target>")]
#[example("discord Pucaet#9528")]
#[example("wynn Pucaet")]
/// Add a discord or wynn partial member with corresponding discord user or mc account.
/// A discord user is specified with their username, ex: "Pucaet" or "Pucaet#9528".
///
/// > **How initial member rank is determined**
/// For discord partial member, the bot attempts to find a rank role from the user and use that,
/// otherwise the lowest rank is used.
/// For wynn partial member, if they're in the guild, their guild rank is used,
/// otherwise the lowest rank is used.
async fn add_partial(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let (profile_type, target_arg) = arg!(ctx, msg, args,
        ProfileType: "Partial member type not provided or is invalid, see `help addPartial` for help",
        ..);

    if let ProfileType::Guild = profile_type {
        finish!(ctx, msg, "Invalid partial member type (need to be discord or wynn)");
    }

    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message guild"));
    let (db, client) = data!(ctx, "db", "reqwest");

    match profile_type {
        ProfileType::Discord => {
            let discord_member = some!(
                ctx!(util::discord::get_member_named(&ctx.http, &guild, target_arg).await)?,
                finish!(ctx, msg, "Failed to find discord user of given name/nick")
            );
            let discord_id = some!(
                memberdb::utils::from_user_id(discord_member.as_ref().user.id),
                cmd_bail!("Failed to convert UserId into DiscordId")
            );

            {
                // checking if there is already a discord profile linked
                let db = db.read().await;
                if let Ok(Some(_)) = memberdb::get_discord_mid(&db, discord_id).await {
                    finish!(ctx, msg, "discord user already linked with a member");
                }
            }

            let rank =
                match memberdb::utils::get_discord_member_rank(&ctx, &guild, &discord_member.as_ref().user).await {
                    Ok(Some(rank)) => rank,
                    _ => memberdb::member::INIT_MEMBER_RANK,
                };

            let result = {
                let db = db.write().await;
                ctx!(
                    memberdb::add_member_discord(&db, discord_id, rank).await,
                    "Failed to add discord partial member"
                )
            };

            finish!(
                ctx,
                msg,
                match result {
                    Ok(_) => "Successfully added discord partial member",
                    Err(_) => "Failed to add discord partial member",
                }
            );
        }
        ProfileType::Wynn => {
            let mcid = ok!(
                wynn::get_ign_id(&client, target_arg).await,
                finish!(ctx, msg, "Provided ign doesn't exist")
            );

            {
                let db = db.read().await;
                if let Ok(Some(_)) = memberdb::get_wynn_mid(&db, &mcid).await {
                    finish!(ctx, msg, "mc account already linked with a member");
                }
            }

            let result = {
                let db = db.write().await;
                let rank = match memberdb::get_guild_rank(&db, &mcid).await {
                    Ok(g_rank) => g_rank.to_member_rank(),
                    Err(_) => memberdb::member::INIT_MEMBER_RANK,
                };
                ctx!(
                    memberdb::add_member_wynn(&db, &mcid, rank, &target_arg).await,
                    "Failed to add wynn partial member"
                )
            };

            finish!(
                ctx,
                msg,
                match result {
                    Ok(_) => "Successfully added wynn partial member",
                    Err(_) => "Failed to add wynn partial member",
                }
            );
        }
        ProfileType::Guild => unreachable!("How?"),
    }
}

#[command("unlink")]
#[only_in(guild)]
#[checks(MainServer, Staff)]
#[usage("<discord | wynn> <target>")]
#[example("discord m:Pucaet")]
#[example("wynn d:Pucaet#9528")]
/// Unlink discord or wynn account from a member which is specified by `target`.
///
/// > **How do I specify different targets**
/// - __Discord user__: "d:<username>", ex: "d:Pucaet" or "d:Pucaet#9528"
/// - __Mc account__: "m:<ign>", ex: "m:SephDark18"
async fn unlink_profile(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let (profile_type, target_arg) = arg!(ctx, msg, args,
        ProfileType: "Profile type not provided or is invalid, see `help unlink` for help",
        ..);

    if let ProfileType::Guild = profile_type {
        finish!(ctx, msg, "Invalid profile type (need to be discord or wynn)");
    }

    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message guild"));
    let (db, client) = data!(ctx, "db", "reqwest");

    let mid = crate::parse_user_target_mid!(ctx, msg, db, client, guild, target_arg);

    let (old_discord, old_mcid) = {
        let db = db.read().await;
        ctx!(memberdb::get_member_links(&db, mid).await)?
    };

    let removed = match profile_type {
        ProfileType::Discord => {
            if old_discord.is_none() {
                finish!(ctx, msg, "There is no linked discord profile for the command to unlink");
            }

            let result = {
                let db = db.write().await;
                ctx!(
                    memberdb::bind_discord(&db, mid, None).await,
                    "Failed to unbind discord profile from member"
                )
            };

            match result {
                Ok(is_removed) => {
                    send!(ctx, msg, "Successfully unlinked discord profile");
                    is_removed
                }
                Err(_) => finish!(ctx, msg, "Failed to unlink discord profile from member"),
            }
        }
        ProfileType::Wynn => {
            if old_mcid.is_none() {
                finish!(ctx, msg, "There is no linked wynn profile for the command to unlink");
            }
            let member_type = {
                let db = db.read().await;
                ctx!(memberdb::get_member_type(&db, mid).await)?
            };
            if let MemberType::GuildPartial = member_type {
                finish!(ctx, msg, "You can't unlink wynn profile of a guild partial member")
            }

            let result = {
                let db = db.write().await;
                ctx!(
                    memberdb::bind_wynn(&db, mid, None, "").await,
                    "Failed to unbind wynn profile from member"
                )
            };

            match result {
                Ok(is_removed) => {
                    send!(ctx, msg, "Successfully unlinked wynn profile");
                    is_removed
                }
                Err(_) => finish!(ctx, msg, "Failed to unlink wynn profile from member"),
            }
        }
        ProfileType::Guild => {
            unreachable!("How?");
        }
    };

    if removed {
        let db = db.read().await;
        db.signal(DBEvent::MemberRemove {
            mid,
            discord_id: old_discord,
            mcid: old_mcid,
        });
    }
    Ok(())
}

#[command("removeMember")]
#[only_in(guild)]
#[checks(MainServer, Staff)]
#[usage("<target>")]
#[example("m:Pucaet")]
#[example("d:Pucaet#9528")]
/// Remove given member specified by `target`.
/// Note that you can't remove a guild partial member with this command.
///
/// > **How do I specify different targets**
/// - __Discord user__: "d:<username>", ex: "d:Pucaet" or "d:Pucaet#9528"
/// - __Mc account__: "m:<ign>", ex: "m:SephDark18"
pub async fn remove_member(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let (db, client) = data!(ctx, "db", "reqwest");

    let mid = crate::parse_user_target_mid!(ctx, msg, db, client, guild, args.rest());

    let result = {
        let db = db.write().await;
        memberdb::remove_member(&db, mid).await
    };

    finish!(
        ctx,
        msg,
        match result {
            Ok(_) => "Successfully removed member",
            Err(why) =>
                if why.is::<DBError>() {
                    // The only DBError can be returned is DBError::WrongMemberType
                    "You can't remove a guild partial member unless they leave the guild"
                } else {
                    error!("Failed to remove member: {:#}", why);
                    "Failed to remove member"
                },
        }
    )
}

async fn set_rank(ctx: &Context, msg: &Message, db: &RwLock<DB>, mid: i64, old_rank: MemberRank, rank: MemberRank) -> CommandResult {
    if old_rank == rank {
        finish!(ctx, msg, "Member is already specified rank");
    }

    let caller_rank = {
        let db = db.read().await;
        let discord_id = some!(
            memberdb::utils::from_user_id(msg.author.id),
            cmd_bail!("Failed to convert UserId to DiscordId")
        );
        let mid = some!(
            ctx!(memberdb::get_discord_mid(&db, discord_id).await)?,
            finish!(ctx, msg, "Only a member can use this command")
        );
        ctx!(memberdb::get_member_rank(&db, mid).await)?
    };
    if caller_rank <= old_rank {
        finish!(ctx, msg, "You can't change the rank of someone with a higher or equal rank to yours")
    }
    if caller_rank >= rank {
        finish!(ctx, msg, "You can't set someone else to a rank that is higher or equal to yours");
    }

    let result = {
        let db = db.write().await;
        memberdb::update_member_rank(&db, mid, rank).await
    };
    finish!(
        ctx,
        msg,
        match result {
            Ok(_) => {
                {
                    let db = db.read().await;
                    db.signal(DBEvent::MemberRankChange { mid, old: old_rank, new: rank });
                }
                "Successfully changed member's rank"
            }
            Err(_) => "Failed to change member's rank",
        }
    )
}

#[command("setRank")]
#[only_in(guild)]
#[checks(MainServer, Staff)]
#[usage("<rank> <target>")]
#[example("Comonaut m:Pucaet")]
#[example("Cadet d:Pucaet#9528")]
/// Set a member's rank.
/// Member is specified by `target`, which can be discord user or ign.
/// `rank` can't be higher or equal to your own rank.
/// The target can't be in a rank that higher or equal to yours.
///
/// There are also shortcut commands: `promote` and `demote`
///
/// > **How do I specify different targets**
/// - __Discord user__: "d:<username>", ex: "d:Pucaet" or "d:Pucaet#9528"
/// - __Mc account__: "m:<ign>", ex: "m:SephDark18"
pub async fn set_member_rank(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let (db, client) = data!(ctx, "db", "reqwest");

    let rank = arg!(ctx, msg, args, MemberRank: "Invalid rank");

    let mid = crate::parse_user_target_mid!(ctx, msg, db, client, guild, args.rest());

    let old_rank = {
        let db = db.read().await;
        ctx!(memberdb::get_member_rank(&db, mid).await)?
    };

    set_rank(&ctx, &msg, &db, mid, old_rank, rank).await
}

#[command("promote")]
#[only_in(guild)]
#[checks(MainServer, Staff)]
#[usage("<target>")]
#[example("m:Pucaet")]
#[example("d:Pucaet#9528")]
/// Promote member to a higher rank.
/// Member is specified by `target`, which can be discord user or ign.
/// `rank` can't be higher or equal to your own rank.
/// The target can't be in a rank that higher or equal to yours.
///
/// There are also the command `setRank` to set a member's rank directly.
///
/// > **How do I specify different targets**
/// - __Discord user__: "d:<username>", ex: "d:Pucaet" or "d:Pucaet#9528"
/// - __Mc account__: "m:<ign>", ex: "m:SephDark18"
pub async fn promote_member(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let (db, client) = data!(ctx, "db", "reqwest");

    let mid = crate::parse_user_target_mid!(ctx, msg, db, client, guild, args.rest());

    let old_rank = {
        let db = db.read().await;
        ctx!(memberdb::get_member_rank(&db, mid).await)?
    };
    let rank = some!(old_rank.promote(),
        finish!(ctx, msg, "Member is already the highest rank"));

    set_rank(&ctx, &msg, &db, mid, old_rank, rank).await
}

#[command("demote")]
#[only_in(guild)]
#[checks(MainServer, Staff)]
#[usage("<target>")]
#[example("m:Pucaet")]
#[example("d:Pucaet#9528")]
/// Demote member to a higher rank.
/// Member is specified by `target`, which can be discord user or ign.
/// `rank` can't be higher or equal to your own rank.
/// The target can't be in a rank that higher or equal to yours.
///
/// There are also the command `setRank` to set a member's rank directly.
///
/// > **How do I specify different targets**
/// - __Discord user__: "d:<username>", ex: "d:Pucaet" or "d:Pucaet#9528"
/// - __Mc account__: "m:<ign>", ex: "m:SephDark18"
pub async fn demote_member(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let (db, client) = data!(ctx, "db", "reqwest");

    let mid = crate::parse_user_target_mid!(ctx, msg, db, client, guild, args.rest());

    let old_rank = {
        let db = db.read().await;
        ctx!(memberdb::get_member_rank(&db, mid).await)?
    };
    let rank = some!(old_rank.demote(),
        finish!(ctx, msg, "Member is already the lowest rank"));

    set_rank(&ctx, &msg, &db, mid, old_rank, rank).await
}

#[command("members")]
#[usage("[filter]")]
#[example("")]
#[example("Chief")]
#[example("guild")]
#[example("<Pilot")]
/// List members with an optional filter.
///
/// `filter` can be following values:
/// full, partial, guild, discord, wynn, Commander, Cosmonaut, Architect, Pilot, Rocketeer, Cadet,
/// Owner, Chief, Strategist, Captain, Recruiter, Recruit.
///
/// Rank filters can also be written as `>Captain` to filter for all guild ranks above captain,
/// or `<Cosmonaut` for all member ranks below cosmonaut.
async fn list_member(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let filter = match args.single::<String>() {
        Ok(s) => Some(ok!(
            MemberFilter::from_str(&s),
            finish!(ctx, msg, "Invalid filter, see `help members` for help")
        )),
        Err(_) => None,
    };

    let db = data!(ctx, "db");

    let table = {
        let db = db.read().await;
        ctx!(memberdb::table::list_members(&ctx, &db, filter).await, "Failed to get members list")?
    };
    if table.len() == 0 {
        finish!(ctx, msg, "Found 0 member");
    }

    let header = vec!["IGN".to_string(), "DISCORD".to_string(), "RANK".to_string()];
    let mut pager = Pager::new(TableData::paginate(table, header, 10));
    ctx!(
        msgtool::interact::page(&ctx, msg.channel_id, &mut pager, 120).await,
        "Error when displaying member list pages"
    )?;

    Ok(())
}

#[command("lb")]
#[usage("<stat> [filter]")]
#[example("weekly_xp")]
#[example("message full")]
#[example("weekly_voice >Pilot")]
#[example("online Recruiter")]
/// Display leader board on specified statistic with an optional filter.
///
/// `stat` can be following values: 
/// message, weekly_message, voice, weekly_voice, online, weekly_online, xp,
/// weekly_xp.
///
/// `filter` can be following values:
/// full, partial, guild, discord, wynn, Commander, Cosmonaut, Architect, Pilot, Rocketeer, Cadet,
/// Owner, Chief, Strategist, Captain, Recruiter, Recruit.
///
/// Rank filters can also be written as `>Captain` to filter for all guild ranks above captain,
/// or `<Cosmonaut` for all member ranks below cosmonaut.
async fn stat_leaderboard(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let stat = arg!(ctx, msg, args, Stat: "Invalid stat, see `help lb` for help");
    let filter = match args.single::<String>() {
        Ok(s) => Some(ok!(
            MemberFilter::from_str(&s),
            finish!(ctx, msg, "Invalid filter, see `help lb` for help")
        )),
        Err(_) => None,
    };

    let db = data!(ctx, "db");

    let (table, header) = {
        let db = db.read().await;
        ctx!(memberdb::table::stat_leaderboard(&ctx, &db, &stat, &filter).await, 
            "Failed to get stat leader board")?
    };
    if table.len() == 0 {
        finish!(ctx, msg, "leader board empty");
    }

    let mut pager = Pager::new(TableData::paginate(table, header, 10));
    ctx!(
        msgtool::interact::page(&ctx, msg.channel_id, &mut pager, 120).await,
        "Error when displaying stat leader board pages"
    )?;

    Ok(())
}

#[command("member")]
#[only_in(guild)]
#[usage("<target>")]
#[example("m:Pucaet")]
#[example("d:Pucaet")]
#[example("d:Pucaet#9528")]
#[example("d:Cosmonaut Pucaet")]
/// Display info of member specified by `target`.
/// If what you want are statistics, use the command `profile` instead.
///
/// > **How do I specify different targets**
/// - __Discord user__: "d:<username>", ex: "d:Pucaet" or "d:Pucaet#9528"
/// - __Mc account__: "m:<ign>", ex: "m:SephDark18"
async fn display_member_info(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let (db, client) = data!(ctx, "db", "reqwest");

    let mid = crate::parse_user_target_mid!(ctx, msg, db, client, guild, args.rest());
    let member = {
        let db = db.read().await;
        some!(
            ctx!(memberdb::get_member(&db, mid).await, "Failed to get member from db")?,
            cmd_bail!("Failed to get member from db")
        )
    };

    let mut content =
        format!("**ID** `{}`\n**Rank** {}\n**Type** {}", member.id, member.rank, member.member_type);

    if let Some(mcid) = member.mcid {
        let db = db.read().await;
        let ign = ctx!(memberdb::get_ign(&db, &mcid).await, "Failed to get wynn.ign")?;
        content.push_str(&format!("\n**Minecraft** {} `{}`", ign, mcid));

        if let Ok(rank) = memberdb::get_guild_rank(&db, &mcid).await {
            content.push_str(&format!("\n**Guild** {}", rank));
        }
    }
    if let Some(id) = member.discord {
        let user = some!(memberdb::utils::to_user(&ctx, id), cmd_bail!("Failed to get discord user"));
        content.push_str(&format!("\n**Discord** {}#{} `{}`", user.name, user.discriminator, id));
    }

    finish!(ctx, msg, content)
}

async fn existing_wynn_link_check(db: &DB, mid: MemberId) -> Option<String> {
    if let Ok((_, Some(old_mcid))) = memberdb::get_member_links(&db, mid).await {
        if let Ok(ign) = memberdb::get_ign(&db, &old_mcid).await {
            return Some(ign);
        }
    }
    None
}

// async fn existing_discord_link_check(db: &DB, mid: MemberId, ctx: &Context, guild: &Guild) -> Option<String> {
//     if let Ok((Some(old_discord), _)) = memberdb::get_member_links(&db, mid).await {
//         let id = some!(memberdb::utils::to_user_id(old_discord), return None);
//         if let Ok(member) = guild.member(&ctx, id).await {
//             return Some(some!(member.nick,
//                 format!("{}#{}", member.user.name, member.user.discriminator)));
//         }
//     }
//     None
// }
