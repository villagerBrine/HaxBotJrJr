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
#[warn(missing_docs, missing_debug_implementations)]
pub mod timer;

use std::sync::Arc;

use serenity::client::{Cache, Context};
use serenity::http::{CacheHttp, Http};
use serenity::model::channel::{GuildChannel, Message};
use serenity::model::guild::{Guild, Member, Role};
use serenity::model::id::{GuildId, RoleId};
use serenity::model::user::User;
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
    /// Member joins the guild
    MemberJoin { member: Member },
    /// Member left the guild
    MemberLeave {
        user: User,
        guild_id: GuildId,
        member: Option<Member>,
    },
    /// Role deleted
    RoleDelete { id: RoleId, role: Option<Role> },
}

signal!(DiscordSignal, DiscordRecv, (DiscordContext, DiscordEvent));
