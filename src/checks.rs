//! Bot command checks
use std::env;

use serenity::framework::standard::macros::check;
use serenity::framework::standard::{Args, CommandOptions, Reason};
use serenity::model::channel::Message;
use serenity::prelude::*;

use util::some;

#[check]
#[name = "MainServer"]
pub async fn main_guild_check(
    _: &Context, msg: &Message, _: &mut Args, _: &CommandOptions,
) -> Result<(), Reason> {
    // Checks if the command is sent in the main guild as specified by `MAIN_GUILD` env var
    let guild_id = some!(
        msg.guild_id,
        return Err(Reason::User("This command can only be used in the main guild".to_string()))
    );
    if guild_id.0.to_string() != env::var("MAIN_GUILD").expect("Expected main guild id in the environment") {
        return Err(Reason::User("This command can only be used in the main guild".to_string()));
    }
    Ok(())
}

#[check]
#[name = "Staff"]
pub async fn is_staff(ctx: &Context, msg: &Message, _: &mut Args, _: &CommandOptions) -> Result<(), Reason> {
    // Checks if the command is called by a staff.
    // A staff if a discord user with the highest group role.
    if let Some(guild) = msg.guild(&ctx) {
        if let Some(role) = memberdb::model::member::MemberRank::Zero.get_group_role(&guild) {
            if let Ok(true) = msg.author.has_role(&ctx, guild.id, role.id).await {
                return Ok(());
            }
        }
    }
    Err(Reason::User("This command can only be used by a staff".to_string()))
}
