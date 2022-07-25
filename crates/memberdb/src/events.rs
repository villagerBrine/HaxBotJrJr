use event::signal;

use crate::discord::DiscordId;
use crate::guild::GuildRank;
use crate::member::{MemberId, MemberRank, MemberType};
use crate::wynn::McId;

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
        removed: bool,
    },
}

signal!(DBSignal, DBRecv, DBEvent);
