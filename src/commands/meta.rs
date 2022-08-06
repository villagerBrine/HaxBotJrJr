//! Commands that displays bot info
use std::collections::HashSet;

use chrono::offset::Utc;
use serenity::client::bridge::gateway::ShardId;
use serenity::framework::standard::macros::{command, help};
use serenity::framework::standard::{help_commands, Args, CommandGroup, CommandResult, HelpOptions};
use serenity::model::channel::Message;
use serenity::model::id::UserId;
use serenity::prelude::*;

use crate::{data, finish};

#[command]
/// ping
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    let shard_manager = data!(ctx, "shard");

    let latency = {
        let manager = shard_manager.lock().await;
        let runners = manager.runners.lock().await;
        match runners.get(&ShardId(ctx.shard_id)) {
            Some(runner) => runner.latency,
            None => finish!(ctx, msg, "No shard found"),
        }
    };

    let content = match latency {
        Some(latency) => format!("`{:?}`", latency),
        None => "Ping not yet available, try again later".to_string(),
    };

    finish!(ctx, msg, content);
}

#[command("utc")]
/// Display current utc time, which is used by the bot's internal timer.
async fn utc_now(ctx: &Context, msg: &Message) -> CommandResult {
    let now = Utc::now();
    finish!(ctx, msg, "{}", now.format("%Y %b %d (%a) %T UTC"));
}

#[help]
#[individual_command_tip = "If you want more information about a specific command, \
just pass the command as argument."]
#[strikethrough_commands_tip_in_guild = ""]
#[strikethrough_commands_tip_in_dm = ""]
#[command_not_found_text = "Could not find: `{}`."]
#[max_levenshtein_distance(3)]
#[indention_prefix = "+"]
#[lacking_permissions = "hide"]
#[lacking_role = "hide"]
#[lacking_conditions = "hide"]
#[wrong_channel = "hide"]
#[embed_success_colour("#9F5BC4")]
pub async fn my_help(
    ctx: &Context, msg: &Message, args: Args, help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup], owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(ctx, msg, args, help_options, groups, owners).await;
    Ok(())
}
