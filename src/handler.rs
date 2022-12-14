//! Bot event handling
use std::env;

use event::{DiscordContext, DiscordEvent, DiscordSignal};
use serenity::async_trait;
use serenity::model::channel::{GuildChannel, Message};
use serenity::model::event::ResumedEvent;
use serenity::model::gateway::Ready;
use serenity::model::guild::{Member, Role};
use serenity::model::id::{GuildId, RoleId};
use serenity::model::user::User;
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use tracing::info;

/// Bot event handler
pub struct Handler {
    discord_signal: DiscordSignal,
    main_guild_id: u64,
}

impl Handler {
    /// Create a new handler
    pub fn new(discord_signal: DiscordSignal) -> Self {
        let main_guild_id: u64 = env::var("MAIN_GUILD")
            .expect("Expected main guild id in the environment")
            .parse()
            .expect("Invalid main guild id");
        Self { discord_signal, main_guild_id }
    }

    /// Broadcast a `DiscordEvent`
    fn send_event(&self, ctx: &Context, event: DiscordEvent) {
        let ctx = DiscordContext::new(ctx, self.main_guild_id);
        self.discord_signal.signal((ctx, event));
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
        // Ensures main guild is cached when event is sent
        std::thread::sleep(std::time::Duration::from_secs(2));
        self.send_event(&ctx, DiscordEvent::Ready);
    }

    async fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        self.send_event(&ctx, DiscordEvent::Message { message: Box::new(msg) });
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        let event = match old {
            Some(old) => {
                if new.channel_id.is_some() {
                    DiscordEvent::VoiceChange {
                        old_state: Box::new(old),
                        new_state: Box::new(new),
                    }
                } else {
                    DiscordEvent::VoiceLeave { old_state: old }
                }
            }
            None => DiscordEvent::VoiceJoin { state: new },
        };
        self.send_event(&ctx, event);
    }

    async fn channel_delete(&self, ctx: Context, channel: &GuildChannel) {
        self.send_event(&ctx, DiscordEvent::ChannelDelete { channel: channel.clone() });
    }

    async fn guild_member_update(&self, ctx: Context, old: Option<Member>, new: Member) {
        self.send_event(&ctx, DiscordEvent::MemberUpdate { old, new });
    }

    async fn guild_role_delete(&self, ctx: Context, _: GuildId, role_id: RoleId, role_data: Option<Role>) {
        self.send_event(&ctx, DiscordEvent::RoleDelete { id: role_id, role: role_data });
    }

    async fn guild_member_addition(&self, ctx: Context, member: Member) {
        self.send_event(&ctx, DiscordEvent::MemberJoin { member });
    }

    async fn guild_member_removal(
        &self, ctx: Context, guild_id: GuildId, user: User, member: Option<Member>,
    ) {
        self.send_event(&ctx, DiscordEvent::MemberLeave { user, guild_id, member });
    }
}
