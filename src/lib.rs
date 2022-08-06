//! Bot utilities
pub mod checks;
pub mod commands;
pub mod data;
pub mod handler;
pub mod hooks;
pub mod logging;
pub mod loops;
pub mod util;

use std::collections::HashSet;
use std::env;
use std::sync::Arc;

use event::DiscordSignal;
use serenity::client::ClientBuilder;
use serenity::framework::StandardFramework;
use serenity::http::Http;
use serenity::model::guild::Guild;
use serenity::model::id::UserId;
use serenity::prelude::*;
use tracing::info;

use crate::handler::Handler;

/// Get the owners of this bot
pub async fn get_owners(http: &Http) -> HashSet<UserId> {
    match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            if let Some(team) = info.team {
                owners.insert(team.owner_user_id);
            } else {
                owners.insert(info.owner.id);
            }
            info!("{:?}", owners);
            owners
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    }
}

/// Build a framework with hooks, prefix and owners already configured
pub async fn my_framework(http: &Http) -> StandardFramework {
    let owners = get_owners(http).await;
    let prefix = env::var("COMMAND_PREFIX").expect("Expected command prefix in the environment");
    StandardFramework::new()
        .configure(|c| c.owners(owners).prefix(prefix))
        .before(crate::hooks::before)
        .after(crate::hooks::after)
        .unrecognised_command(crate::hooks::unknown_command)
        .on_dispatch_error(crate::hooks::dispatch_error)
}

/// Build a client with intents and event handler already configured
pub fn my_client(token: &str, framework: StandardFramework, discord_signal: DiscordSignal) -> ClientBuilder {
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_VOICE_STATES;
    Client::builder(token, intents)
        .framework(framework)
        .event_handler(Handler::new(discord_signal))
}

/// This function waits for the bot to be ready, and return the main guild object (as specified by
/// the `MAIN_GUILD` env var)
pub async fn wait_main_guild(signal: DiscordSignal) -> Arc<Guild> {
    let mut receiver = signal.connect();
    let event = receiver.recv().await.unwrap();
    event.0.main_guild.clone()
}
