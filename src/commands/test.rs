use serenity::client::Context;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::Message;
use tracing::error;

use memberdb::model::discord::DiscordId;
use util::{ctx, some};
use crate::util::db::{self, TargetId};

#[command("rollitems")]
#[only_in(guild)]
#[checks(Staff)]
/// Change your custom nick to `custome_nick`.
/// Custom nick is the part of your nick that is after your rank and ign/discord username,
/// for example: "❈ Pucaet This part is my custom nick".
///
/// You have to be a member to use this command.
async fn rngroll(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let db = data!(ctx, "db");
    let profiles = {
        let db = db.read().await;
        let guild = {
           let db = db.read().await;
           ctx!(memberdb::table::list_members(&ctx.cache, &db, &filters).await, "Failed to get members list")?
        };
        for member in guild {
          match member {
              TargetId::Discord(id) => {
                  let id = DiscordId::try_from(id.0)?;
                  Profiles::from_discord(&db, id).await
              }
              TargetId::Wynn(id) => Profiles::from_mc(&db, &id).await,
          }
      }
    };
  if profiles.is_done() {
        finish!(ctx, msg, "No profiles found");
  }
}
