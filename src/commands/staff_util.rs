use serenity::client::Context;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::Message;
use tracing::error;

use util::{ctx, some};

use crate::checks::STAFF_CHECK;
use crate::{cmd_bail, data, finish};

#[command("fixNick")]
#[only_in(guild)]
#[checks(Staff)]
#[usage("<discord_user>")]
#[example("Pucaet")]
#[example("Pucaet#9528")]
/// Fix `discord_user`'s nickname.
/// `discord_user` is a discord username, ex: "Pucaet" or "Pucaet#9528".
///
/// Note that this command works even if target user has the `NoNickUpdate` tag.
async fn fix_nick(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let db = data!(ctx, "db");

    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let discord_member = some!(
        ctx!(util::discord::get_member_named(&ctx.http, &guild, args.rest()).await)?,
        finish!(ctx, msg, "Can't find specified discord user")
    );

    let discord_id = some!(
        memberdb::utils::from_user_id(discord_member.as_ref().user.id),
        cmd_bail!("Failed to convert user id to discord id")
    );
    let mid = {
        let db = db.read().await;
        some!(
            ctx!(memberdb::get_discord_mid(&db, discord_id).await)?,
            finish!(ctx, msg, "The provided discord user isn't a member")
        )
    };

    let result =
        crate::util::discord::fix_member_nick(&ctx.http, &db, mid, discord_member.as_ref(), None).await;

    finish!(
        ctx,
        msg,
        match result {
            Ok(_) => "done",
            Err(why) => {
                error!("Failed to change nickname: {:#}", why);
                "Unable to change nickname"
            }
        }
    )
}

#[command("fixRole")]
#[only_in(guild)]
#[checks(Staff)]
#[usage("<discord_user>")]
#[example("Pucaet")]
#[example("Pucaet#9528")]
/// Fix `discord_user`'s role.
/// `discord_user` is a discord username, ex: "Pucaet" or "Pucaet#9528".
///
/// Note that this command works even if target user has the `NoRoleUpdate` tag.
async fn fix_role(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let db = data!(ctx, "db");

    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let discord_member = some!(
        ctx!(util::discord::get_member_named(&ctx.http, &guild, args.rest()).await)?,
        finish!(ctx, msg, "Can't find specified discord user")
    );

    let discord_id = some!(
        memberdb::utils::from_user_id(discord_member.as_ref().user.id),
        cmd_bail!("Failed to convert user id to discord id")
    );

    let rank = {
        let db = db.read().await;
        let mid = some!(
            ctx!(memberdb::get_discord_mid(&db, discord_id).await)?,
            finish!(ctx, msg, "The provided discord user isn't a member")
        );
        ctx!(memberdb::get_member_rank(&db, mid).await)?
    };

    let mut discord_member = discord_member.into_owned();
    let result =
        ctx!(crate::util::discord::fix_discord_roles(&ctx.http, rank, &guild, &mut discord_member).await);

    finish!(
        ctx,
        msg,
        match result {
            Ok(_) => "Done",
            Err(_) => "Failed to update roles",
        }
    )
}

#[command("syncIgn")]
#[only_in(guild)]
#[checks(Staff)]
#[usage("<ign>")]
#[example("Pucaet")]
/// Because the bot only updates ign of in-game guild members,
/// this command is need to update ign of in-game non-guild members.
///
/// Note that the `ign` is the ign that is currently stored in database.
async fn sync_member_ign(ctx: &Context, msg: &Message, arg: Args) -> CommandResult {
    let (db, client) = data!(ctx, "db", "reqwest");

    let old_ign = arg.rest();
    let mcid = {
        let db = db.read().await;
        ctx!(memberdb::get_ign_mcid(&db, old_ign).await)?
    };
    let mcid = some!(mcid, finish!(ctx, msg, "Unable to find the mc account in database"));

    let ign = ctx!(wynn::get_ign(&client, &mcid).await)?;
    if ign == old_ign {
        finish!(ctx, msg, "Ign unchanged");
    }

    {
        let db = db.write().await;
        ctx!(memberdb::update_ign(&db, &mcid, &ign).await)?
    }

    finish!(ctx, msg, "Ign updated to {}", ign)
}

#[command("rankSymbol")]
/// Display all rank symbols
async fn get_rank_symbols(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    let mut content = String::new();

    for rank in memberdb::member::MEMBER_RANKS {
        content.push_str(&format!("**{}** `{}`\n", rank, rank.get_symbol()));
    }

    finish!(ctx, msg, content)
}

// TODO move this command to another file
#[command("nick")]
#[only_in(guild)]
#[usage("<custom_nick>")]
#[example("my custom nick")]
/// Change your custom nick to `custome_nick`.
/// Custom nick is the part of your nick that is after your rank and ign/discord username,
/// for example: "❈ Pucaet This part is my custom nick".
///
/// You have to be a member to use this command.
async fn set_custom_nick(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let custom_nick = args.rest();

    let db = data!(ctx, "db");

    let discord_member = ctx!(msg.member(&ctx).await, "Failed to get member who sent the message")?;
    let discord_id = ctx!(i64::try_from(discord_member.user.id), "Failed to convert user id to discord id")?;
    let mid = {
        let db = db.read().await;
        some!(
            ctx!(memberdb::get_discord_mid(&db, discord_id).await)?,
            finish!(ctx, msg, "You aren't a member")
        )
    };

    let result =
        crate::util::discord::fix_member_nick(&ctx.http, &db, mid, &discord_member, Some(custom_nick)).await;

    finish!(
        ctx,
        msg,
        match result {
            Ok(_) => "done",
            Err(why) => {
                error!("Failed to change nickname: {:#}", why);
                "Unable to change nickname"
            }
        }
    )
}
