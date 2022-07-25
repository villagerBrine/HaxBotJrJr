use std::env;

use event::{DiscordContext, DiscordEvent, DiscordSignal};
use serenity::async_trait;
use serenity::model::channel::{GuildChannel, Message};
use serenity::model::event::ResumedEvent;
use serenity::model::gateway::Ready;
use serenity::model::guild::{Member, Role};
use serenity::model::id::{GuildId, RoleId};
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use tracing::info;

pub struct Handler {
    discord_signal: DiscordSignal,
    main_guild_id: u64,
}

impl Handler {
    pub fn new(discord_signal: DiscordSignal) -> Self {
        let main_guild_id: u64 = env::var("MAIN_GUILD")
            .expect("Expected main guild id in the environment")
            .parse()
            .expect("Invalid main guild id");
        Self { discord_signal, main_guild_id }
    }
}

macro_rules! send_event {
    ($self:expr, $ctx:expr, $event:expr) => {{
        let ctx = DiscordContext::new($ctx, $self.main_guild_id);
        $self.discord_signal.signal((ctx, $event))
    }};
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
        // Ensures main guild is cached when event is sent
        std::thread::sleep(std::time::Duration::from_secs(2));
        send_event!(self, &ctx, DiscordEvent::Ready);
    }

    async fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        send_event!(self, &ctx, DiscordEvent::Message { message: msg });
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        info!(?old, ?new, "voice state update");
        let event = match old {
            Some(old) => {
                if new.channel_id.is_some() {
                    DiscordEvent::VoiceChange { old_state: old, new_state: new }
                } else {
                    DiscordEvent::VoiceLeave { old_state: old }
                }
            }
            None => DiscordEvent::VoiceJoin { state: new },
        };
        send_event!(self, &ctx, event);
    }

    async fn channel_delete(&self, ctx: Context, channel: &GuildChannel) {
        send_event!(self, &ctx, DiscordEvent::ChannelDelete { channel: channel.clone() });
    }

    async fn guild_member_update(&self, ctx: Context, old: Option<Member>, new: Member) {
        send_event!(self, &ctx, DiscordEvent::MemberUpdate { old, new });
    }

    async fn guild_role_delete(&self, ctx: Context, _: GuildId, role_id: RoleId, role_data: Option<Role>) {
        send_event!(self, &ctx, DiscordEvent::RoleDelete { id: role_id, role: role_data });
    }
}
