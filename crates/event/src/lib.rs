/// Signals are wrapped broadcast channels for broadcasting events
pub mod timer;

use std::sync::Arc;

use serenity::client::{Cache, Context};
use serenity::http::{CacheHttp, Http};
use serenity::model::channel::{GuildChannel, Message};
use serenity::model::guild::{Guild, Member, Role};
use serenity::model::id::RoleId;
use serenity::model::voice::VoiceState;

#[macro_export]
/// Create an event signal and receiver for an event type
macro_rules! signal {
    ($sig_name:ident, $recv_name:ident, $event:ty) => {
        #[derive(Debug, Clone)]
        pub struct $sig_name(std::sync::Arc<tokio::sync::broadcast::Sender<std::sync::Arc<$event>>>);
        pub type $recv_name = tokio::sync::broadcast::Receiver<std::sync::Arc<$event>>;

        impl $sig_name {
            pub fn new(capacity: usize) -> Self {
                let (sender, _) = tokio::sync::broadcast::channel(capacity);
                Self(std::sync::Arc::new(sender))
            }

            /// Return a receiver for this signal
            pub fn connect(&self) -> tokio::sync::broadcast::Receiver<std::sync::Arc<$event>> {
                self.0.subscribe()
            }

            /// Broadcast an event
            pub fn signal(&self, data: $event) -> usize {
                self.0.send(std::sync::Arc::new(data)).unwrap()
            }
        }
    };
}

#[derive(Debug, Clone)]
pub enum WynnEvent {
    MemberJoin {
        id: String,
        rank: String,
        ign: String,
        xp: i64,
    },
    MemberLeave {
        id: String,
        rank: String,
        ign: String,
    },
    MemberRankChange {
        id: String,
        ign: String,
        old_rank: String,
        new_rank: String,
    },
    MemberContribute {
        id: String,
        ign: String,
        old_contrib: i64,
        new_contrib: i64,
    },
    MemberNameChange {
        id: String,
        old_name: String,
        new_name: String,
    },
    GuildLevelUp {
        level: u8,
    },
    PlayerJoin {
        ign: String,
        world: String,
    },
    PlayerStay {
        ign: String,
        world: String,
        elapsed: u64,
    },
    PlayerLeave {
        ign: String,
    },
}

// Because wynncraft events are created in bulk by the api loop, so it is wrapped in `Vec` so it
// can also be broadcast in bulk.
signal!(WynnSignal, WynnRecv, Vec<WynnEvent>);

#[derive(Debug, Clone)]
/// Useful objects to be attached to discord events
pub struct DiscordContext {
    pub http: Arc<Http>,
    pub cache: Arc<Cache>,
    pub main_guild: Arc<Guild>,
}

impl DiscordContext {
    pub fn new(ctx: &Context, main_guild_id: u64) -> Self {
        let http = ctx.http.clone();
        let cache = ctx.cache.clone();
        let main_guild = Arc::new(cache.guild(main_guild_id).expect("Unable to find main guild"));
        Self { http, cache, main_guild }
    }
}

impl CacheHttp for DiscordContext {
    fn http(&self) -> &Http {
        &self.http
    }

    fn cache(&self) -> Option<&Arc<Cache>> {
        Some(&self.cache)
    }
}

#[derive(Debug, Clone)]
pub enum DiscordEvent {
    Ready,
    Message {
        message: Message,
    },
    VoiceJoin {
        state: VoiceState,
    },
    VoiceLeave {
        old_state: VoiceState,
    },
    VoiceChange {
        old_state: VoiceState,
        new_state: VoiceState,
    },
    ChannelDelete {
        channel: GuildChannel,
    },
    MemberUpdate {
        old: Option<Member>,
        new: Member,
    },
    RoleDelete {
        id: RoleId,
        role: Option<Role>,
    },
}

signal!(DiscordSignal, DiscordRecv, (DiscordContext, DiscordEvent));
