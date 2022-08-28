//! Provides the `signal` macro for creating event broadcast channels
//! ```
//! use std::sync::Arc;
//! use event::signal;
//!
//! // Signals are created using the `signal` macro
//! #[derive(Debug)]
//! pub struct KeyPressEvent(char);
//!
//! signal!(KeyPressSignal, KeyPressRecv, KeyPressEvent);
//!
//! // Creating signal
//! let signal = KeyPressSignal::new(16);
//!
//! // Sending events through signal
//! async fn press_key(signal: KeyPressSignal, key: char, amount: u64) {
//!     for _ in 0..amount {
//!         signal.signal(KeyPressEvent(key));
//!     }
//! }
//!
//! //Creating receiver, and receiving events through it
//! async fn key_press_listening_loop(signal: KeyPressSignal) {
//!     let mut receiver: KeyPressRecv = signal.connect();
//!     loop {
//!         let key: Arc<KeyPressEvent> = receiver.recv().await.expect("Too much key presses!");
//!         println!("Pressed {}", key.0);
//!     }
//! }
//! ```
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
///
/// Takes a name for the signal, a name for the receiver, and the type of event.
/// ```
/// use event::signal;
///
/// #[derive(Debug)]
/// struct KeyPressEvent(char);
///
/// signal!(KeyPressSignal, KeyPressRecv, KeyPressEvent);
/// ```
/// The receiver type it creates is a type alias of [`Receiver`] that receives the event type
/// wrapped in [`Arc`].
///
/// [`Receiver`]: tokio::sync::broadcast::Receiver
/// [`Arc`]: std::sync::Arc
macro_rules! signal {
    ($sig_name:ident, $recv_name:ident, $event:ty) => {
        /// Event signal
        #[derive(Debug, Clone)]
        pub struct $sig_name(::std::sync::Arc<::tokio::sync::broadcast::Sender<::std::sync::Arc<$event>>>);

        /// Event receiver
        ///
        /// Type alias of [`Receiver`] that receives the event type wrapped in [`Arc`].
        ///
        /// [`Receiver`]: tokio::sync::broadcast::Receiver
        /// [`Arc`]: std::sync::Arc
        pub type $recv_name = ::tokio::sync::broadcast::Receiver<::std::sync::Arc<$event>>;

        impl $sig_name {
            /// Create a new signal
            pub fn new(capacity: usize) -> Self {
                let (sender, _) = ::tokio::sync::broadcast::channel(capacity);
                Self(::std::sync::Arc::new(sender))
            }

            /// Return a receiver for this signal
            pub fn connect(&self) -> ::tokio::sync::broadcast::Receiver<::std::sync::Arc<$event>> {
                self.0.subscribe()
            }

            /// Broadcast an event
            pub fn signal(&self, data: $event) -> usize {
                self.0.send(::std::sync::Arc::new(data)).unwrap()
            }
        }
    };
}

/// Wynncraft/Mojang events
#[derive(Debug, Clone)]
pub enum WynnEvent {
    /// New guild member joined
    MemberJoin {
        id: String,
        rank: String,
        ign: String,
        xp: i64,
    },
    /// Guild member left the guild
    MemberLeave { id: String, rank: String, ign: String },
    /// Guild member's rank changed
    MemberRankChange {
        id: String,
        ign: String,
        old_rank: String,
        new_rank: String,
    },
    /// Guild member contributed xp
    MemberContribute {
        id: String,
        ign: String,
        old_contrib: i64,
        new_contrib: i64,
    },
    /// Guild member's ign changed
    ///
    /// Note that this event won't emit for players that aren't in the in-game guild.
    MemberNameChange {
        id: String,
        old_name: String,
        new_name: String,
    },
    /// Guild's level changed
    ///
    /// This event only includes the guild's new level.
    GuildLevelUp { level: u8 },
    /// Player joins the server
    ///
    /// Note that this is only emitted for when a player logs on.
    PlayerJoin { ign: String, world: String },
    /// Player stays on the server
    ///
    /// This event are sent per minute while the player is on the server.
    /// The [`elapsed`] field contains the amount of seconds the player had spend on the server
    /// measured from the previous [`PlayerJoin`] or [`PlayerStay`] event.
    ///
    /// [`elapsed`]: crate::WynnEvent::PlayerStay::elapsed
    /// [`PlayerJoin`]: crate::WynnEvent::PlayerJoin
    /// [`PlayerStay`]: crate::WynnEvent::PlayerStay
    PlayerStay {
        ign: String,
        world: String,
        elapsed: u64,
    },
    /// Player moved from one server to another
    PlayerMove {
        ign: String,
        old_world: String,
        new_world: String,
    },
    /// Players logs off
    ///
    /// [`world`] is the server the player logged off from.
    ///
    /// [`world`]: crate::WynnEvent::PlayerLeave::world
    PlayerLeave { ign: String, world: String },
}

// Because wynncraft events are created in bulk by the api loop, so it is wrapped in `Vec` so it
// can also be broadcast in bulk.
signal!(WynnSignal, WynnRecv, Vec<WynnEvent>);

/// Useful data to be broadcasted alongside discord events
#[derive(Debug, Clone)]
pub struct DiscordContext {
    pub http: Arc<Http>,
    pub cache: Arc<Cache>,
    pub main_guild: Arc<Guild>,
}

impl DiscordContext {
    /// Creates a new event context
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

/// Discord events
#[derive(Debug, Clone)]
pub enum DiscordEvent {
    /// The bot is ready
    Ready,
    /// A message was send
    Message { message: Box<Message> },
    /// User joined a vc
    VoiceJoin { state: VoiceState },
    /// User left a vc
    VoiceLeave { old_state: VoiceState },
    /// User changed its voice state (ex: mute/unmute)
    VoiceChange {
        old_state: Box<VoiceState>,
        new_state: Box<VoiceState>,
    },
    /// Guild channel deleted
    ChannelDelete { channel: GuildChannel },
    /// Guild member updated (ex: nick change)
    MemberUpdate { old: Option<Member>, new: Member },
    /// Member joins a guild
    MemberJoin { member: Member },
    /// Role deleted
    RoleDelete { id: RoleId, role: Option<Role> },
}

signal!(DiscordSignal, DiscordRecv, (DiscordContext, DiscordEvent));
