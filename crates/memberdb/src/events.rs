//! Database events
use event::signal;

use crate::model::discord::DiscordId;
use crate::model::guild::GuildRank;
use crate::model::member::{MemberId, MemberRank, MemberType};
use crate::model::wynn::McId;

#[derive(Debug, Clone)]
pub enum DBEvent {
    MemberAdd {
        mid: MemberId,
        discord_id: Option<DiscordId>,
        mcid: Option<McId>,
        rank: MemberRank,
    },
    MemberRemove {
        mid: MemberId,
        discord_id: Option<DiscordId>,
        mcid: Option<McId>,
    },
    MemberFullPromote {
        mid: MemberId,
        before: MemberType,
    },
    MemberAutoGuildDemote {
        mid: MemberId,
        before: MemberType,
    },
    MemberRankChange {
        mid: MemberId,
        old: MemberRank,
        new: MemberRank,
    },
    WynnProfileAdd {
        mcid: McId,
        mid: Option<MemberId>,
    },
    WynnProfileBind {
        mid: MemberId,
        old: Option<McId>,
        new: McId,
    },
    WynnProfileUnbind {
        mid: MemberId,
        before: McId,
        // Indicates if the member was/about to be removed
        removed: bool,
    },
    GuildProfileAdd {
        mcid: McId,
        rank: GuildRank,
    },
    DiscordProfileAdd {
        discord_id: DiscordId,
        mid: Option<MemberId>,
    },
    DiscordProfileBind {
        mid: MemberId,
        old: Option<DiscordId>,
        new: DiscordId,
    },
    DiscordProfileUnbind {
        mid: MemberId,
        before: DiscordId,
        // Indicates if the member was/about to be removed
        removed: bool,
    },
    WeeklyReset {
        // All the weekly leaderboards before the reset
        message_lb: (Vec<Vec<String>>, Vec<String>),
        voice_lb: (Vec<Vec<String>>, Vec<String>),
        online_lb: (Vec<Vec<String>>, Vec<String>),
        xp_lb: (Vec<Vec<String>>, Vec<String>),
    },
}

signal!(DBSignal, DBRecv, DBEvent);
