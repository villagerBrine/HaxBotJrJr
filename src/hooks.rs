//! Bot hooks
use serenity::framework::standard::macros::hook;
use serenity::framework::standard::{CommandResult, DispatchError, Reason};
use serenity::model::channel::{Channel, Message, PermissionOverwriteType};
use serenity::model::permissions::Permissions;
use serenity::prelude::*;
use tracing::{error, info};

use util::{ok, some};

#[hook]
pub async fn before(ctx: &Context, msg: &Message, _: &str) -> bool {
    let channel = ok!(msg.channel(&ctx).await, return false);

    // If a command is called in a guild channel, then this check is performed to determine if that
    // channel is configured to allow command usages.
    if let Channel::Guild(channel) = channel {
        let guild = some!(msg.guild(&ctx), return false);
        // A channel is configured to allow command usages if the bot user's SEND_MESSAGES
        // permission is set to "allow" within the channel permisions.
        let kind = PermissionOverwriteType::Member(ctx.cache.current_user_id());
        let allow = util::discord::check_channel_allow(&guild, &channel, kind, Permissions::SEND_MESSAGES);
        if allow {
            info!("Invoking command '{}' by user '{}' in '{}'", msg.content, msg.author.name, channel);
        }
        return allow;
    }

    info!("Invoking command '{}' by user '{}' in '{}'", msg.content, msg.author.name, channel);
    true
}

#[hook]
pub async fn after(ctx: &Context, msg: &Message, command_name: &str, command_result: CommandResult) {
    // Report unhandled error
    if let Err(why) = command_result {
        error!("Command '{}' returned error: {}", command_name, why);
        let _ = msg
            .reply(&ctx, "**Encountered an unexpected error when running command**")
            .await;
    }
}

#[hook]
pub async fn unknown_command(ctx: &Context, msg: &Message, unknown_command_name: &str) {
    let content = format!("Could not find command named '{}'", unknown_command_name);
    let _ = msg.reply(&ctx, content).await;
}

#[hook]
pub async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError, _: &str) {
    let _ = match error {
        DispatchError::BlockedChannel => msg.reply(&ctx, "You can't use command in a blocked channel").await,
        DispatchError::BlockedUser => msg.reply(&ctx, "You are blocked from using command").await,
        DispatchError::CheckFailed(_, reason) => match reason {
            Reason::User(s) => msg.reply(&ctx, format!("You can't use this command: {}", s)).await,
            _ => msg.reply(&ctx, "You can't use this command").await,
        },
        DispatchError::CommandDisabled => msg.reply(&ctx, "This command is disabled").await,
        DispatchError::LackingPermissions(perms) => {
            msg.reply(&ctx, format!("To use this command you need following permisions: {}", perms))
                .await
        }
        DispatchError::LackingRole => {
            msg.reply(&ctx, "You do not have the required role to use this command".to_string())
                .await
        }
        DispatchError::OnlyForGuilds => msg.reply(&ctx, "This command can only be used in a server").await,
        DispatchError::OnlyForOwners => msg.reply(&ctx, "This command can only be used by bot owner").await,
        DispatchError::Ratelimited(info) => {
            msg.reply(
                &ctx,
                format!("This command is used too frequently, try again in {}s", info.rate_limit.as_secs()),
            )
            .await
        }
        _ => msg.reply(&ctx, "Unable to run this command").await,
    };
}
