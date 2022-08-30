//! Discord nickname related commands
use serenity::client::Context;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::Message;
use tracing::error;

use util::{ctx, some};

use crate::{data, finish};

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
            ctx!(memberdb::get_discord_mid(&mut db.exe(), discord_id).await)?,
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
