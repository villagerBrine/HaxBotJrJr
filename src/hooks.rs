use serenity::framework::standard::macros::hook;
use serenity::framework::standard::{CommandResult, DispatchError, Reason};
use serenity::model::channel::{Channel, Message, PermissionOverwriteType};
use serenity::model::permissions::Permissions;
use serenity::prelude::*;
use tracing::{error, info};

use util::ok;

#[hook]
pub async fn before(ctx: &Context, msg: &Message, _: &str) -> bool {
    let channel = ok!(msg.channel(&ctx).await, return false);
    if let Channel::Guild(channel) = channel {
        let kind = PermissionOverwriteType::Member(ctx.cache.current_user_id());
        for perm in channel.permission_overwrites {
            if perm.kind == kind && perm.allow == Permissions::SEND_MESSAGES {
                info!("Invoking command '{}' by user '{}'", msg.content, msg.author.name);
                return true;
            }
        }
        return false;
    }

    info!("Invoking command '{}' by user '{}'", msg.content, msg.author.name);
    true
}

#[hook]
pub async fn after(ctx: &Context, msg: &Message, command_name: &str, command_result: CommandResult) {
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
    let _ = msg
        .reply(
            &ctx,
            match error {
                DispatchError::BlockedChannel => "You can't use command in a blocked channel".to_string(),
                DispatchError::BlockedUser => "You are blocked from using command".to_string(),
                DispatchError::CheckFailed(_, reason) => match reason {
                    Reason::User(s) => {
                        format!("You can't use this command: {}", s)
                    }
                    _ => "You can't use this command".to_string(),
                },
                DispatchError::CommandDisabled => "This command is disabled".to_string(),
                DispatchError::LackingPermissions(perms) => {
                    format!("To use this command you need following permisions: {}", perms)
                }
                DispatchError::LackingRole => {
                    "You do not have the required role to use this command".to_string()
                }
                DispatchError::OnlyForGuilds => "This command can only be used in a server".to_string(),
                DispatchError::OnlyForOwners => "This command can only be used by bot owner".to_string(),
                DispatchError::Ratelimited(info) => format!(
                    "This command is used too frequently, try again in {}s",
                    info.rate_limit.as_secs()
                ),
                _ => "Unable to run this command".to_string(),
            },
        )
        .await;
}
