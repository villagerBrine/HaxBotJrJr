//! Staf util commands
use std::collections::HashSet;
use std::fmt::Write as _;

use serenity::client::Context;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::Message;
use tracing::error;

use memberdb::model::discord::DiscordId;
use memberdb::model::wynn::McId;
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

    let username = args.rest();
    if username.is_empty() {
        finish!(ctx, msg, "Target discord user not provided")
    }

    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let discord_member = some!(
        ctx!(util::discord::get_member_named(&ctx.http, &guild, username).await)?,
        finish!(ctx, msg, "Can't find specified discord user")
    );

    let discord_id = DiscordId::try_from(discord_member.as_ref().user.id.0)?;
    let mid = {
        let db = db.read().await;
        some!(
            ctx!(discord_id.mid(&mut db.exe()).await)?,
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

    let discord_id = DiscordId::try_from(discord_member.as_ref().user.id.0)?;

    let rank = {
        let db = db.read().await;
        let mid = some!(
            ctx!(discord_id.mid(&mut db.exe()).await)?,
            finish!(ctx, msg, "The provided discord user isn't a member")
        );
        ctx!(mid.rank(&mut db.exe()).await)?
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
#[bucket("mojang")]
#[only_in(guild)]
#[checks(Staff)]
#[usage("<ign>")]
#[example("Pucaet")]
/// Because the bot only updates ign of in-game guild members,
/// this command is need to update ign of in-game non-guild members.
///
/// Note that the `ign` is the ign that is currently stored in database.
/// For example is a player named "old_name" (as stored in the database) changed their name, then
/// you need to use the command `syncIgn old_name` to update their ign in the database.
async fn sync_member_ign(ctx: &Context, msg: &Message, arg: Args) -> CommandResult {
    let (db, client) = data!(ctx, "db", "reqwest");

    let old_ign = arg.rest();
    let mcid = {
        let db = db.read().await;
        ctx!(McId::from_ign(&mut db.exe(), old_ign).await)?
    };
    let mcid = some!(mcid, finish!(ctx, msg, "Unable to find the mc account in database"));

    let ign = ctx!(wynn::get_ign(&client, &mcid.0).await)?;
    if ign == old_ign {
        finish!(ctx, msg, "Ign unchanged");
    }

    {
        let db = db.write().await;
        let mut tx = ctx!(db.begin().await)?;
        ctx!(mcid.set_ign(&mut tx, &ign).await)?;
        ctx!(tx.commit().await)?;
    }

    finish!(ctx, msg, "Ign updated to {}", ign)
}

#[command("rankSymbol")]
/// Display all rank symbols
async fn get_rank_symbols(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    let mut content = String::new();

    for rank in memberdb::model::member::MEMBER_RANKS {
        writeln!(content, "**{}** `{}`", rank, rank.get_symbol())?;
    }

    finish!(ctx, msg, content)
}

#[command("igns")]
#[usage("[omits]")]
#[example("")]
#[example("Pucaet SephDark18")]
/// List all member igns.
///
/// If a list of igns are provided, then those igns are omitted from the output list.
/// For example `igns Pucaet` would output all member igns except `Pucaet`.
async fn list_igns(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let omit: HashSet<&str> = args.raw().collect();

    let db = data!(ctx, "db");

    let mut igns = {
        let db = db.read().await;
        McId::igns(&mut db.exe()).await?
    };
    igns.sort_by_key(|s| s.to_ascii_lowercase());

    let content = if omit.is_empty() {
        let igns = igns.join(" ");
        format!("`{}`", igns)
    } else {
        let mut content = "`".to_string();
        let igns = igns.iter().filter(|s| !omit.contains(&s.as_str()));
        for ign in igns {
            write!(content, "{} ", ign)?;
        }
        content.push('`');
        content
    };

    finish!(ctx, msg, content)
}
