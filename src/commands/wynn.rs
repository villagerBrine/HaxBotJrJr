use anyhow::Context as AHContext;
use serenity::client::Context;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::Message;

use crate::{data, finish, send_embed};

#[command("online")]
/// Display online members
async fn display_online_players(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    let cache = data!(ctx, "cache");

    {
        let online = cache.online.read().await;
        if online.0.is_empty() {
            finish!(ctx, msg, "No players are online");
        }

        // 25 is the embed field limit
        if online.0.len() > 25 {
            let mut content = String::new();
            for (world, igns) in online.0.iter() {
                content.push_str("**");
                content.push_str(world);
                content.push_str("**: ");
                for ign in igns {
                    content.push_str(ign);
                    content.push(' ');
                }
                content.push('\n');
            }
            finish!(ctx, msg, content);
        } else {
            send_embed!(ctx, msg, |e| {
                for (world, igns) in online.0.iter() {
                    let igns = igns.iter().map(|s| s.as_str()).collect::<Vec<&str>>().join(" ");
                    e.field(world, igns, true);
                }
                e
            });
        }
    }

    Ok(())
}
