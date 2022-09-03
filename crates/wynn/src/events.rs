//! Provides [`WynnEvent`] and types to send/receive it.
//!
//! Use [`WynnSignal`] to send events, and [`WynnRecv`] to receive the. See [`event`] for more
//! info.
//!
//! You don't need to broadcast events yourself, this crate provides the function [`start_loops`]
//! for starting event broadcasting loops.
//!
//! [`start_loops`]: crate::loops::start_loops
use event::signal;

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
