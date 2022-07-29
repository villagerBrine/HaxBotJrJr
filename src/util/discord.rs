use anyhow::Result;
use serenity::http::Http;
use serenity::model::guild::{Guild, Member};
use tokio::sync::RwLock;

use memberdb::member::{MemberId, MemberRank};
use memberdb::DB;
use util::ctx;

/// Update discord member's role to match up with their rank
pub async fn fix_discord_roles(
    http: &Http, rank: MemberRank, guild: &Guild, member: &mut Member,
) -> Result<()> {
    if memberdb::member::MANAGED_MEMBER_RANKS.contains(&rank) {
        util::discord::add_role(&http, rank.get_role(&guild), member).await?;
        util::discord::add_role(&http, rank.get_group_role(&guild), member).await?;
    }

    for other_rank in memberdb::member::MANAGED_MEMBER_RANKS {
        if rank == other_rank {
            continue;
        }

        util::discord::remove_role(&http, other_rank.get_role(&guild), member).await?;
        if !rank.is_same_group(other_rank) {
            util::discord::remove_role(&http, other_rank.get_group_role(&guild), member).await?;
        }
    }

    Ok(())
}

/// Given discord nick, return custom nick within it
pub fn extract_custom_nick(nick: &str) -> &str {
    match nick.find(' ') {
        Some(no_rank_i) => match &nick[no_rank_i + 1..].find(' ') {
            Some(nick_i) => &nick[nick_i + 1..],
            None => "",
        },
        None => "",
    }
}

/// Fix a member's discord nick.
/// If `custom_nick` is none, attempts to preserve their original custom nick.
pub async fn fix_member_nick(
    http: &Http, db: &RwLock<DB>, mid: MemberId, discord_member: &Member, custom_nick: Option<&str>,
) -> Result<Member> {
    let rank = {
        let db = db.read().await;
        ctx!(memberdb::get_member_rank(&db, mid).await)?
    };
    let ign = {
        let db = db.read().await;
        let (_, mcid) = ctx!(memberdb::get_member_links(&db, mid).await)?;
        match mcid {
            Some(mcid) => memberdb::get_ign(&db, &mcid).await.ok(),
            None => None,
        }
    };
    // convert Option<String> to Option<&String>
    let ign = match &ign {
        Some(ign) => Some(ign),
        None => None,
    };

    fix_discord_nick(http, &rank, ign, discord_member, custom_nick).await
}

/// Fix a discord member's nick.
/// If `custom_nick` is none, attempts to preserve their original custom nick.
pub async fn fix_discord_nick(
    http: &Http, rank: &MemberRank, ign: Option<&String>, discord_member: &Member, custom_nick: Option<&str>,
) -> Result<Member> {
    let name = match ign {
        Some(ign) => ign,
        None => &discord_member.user.name,
    };
    let custom_nick = match custom_nick {
        Some(s) => s,
        None => match &discord_member.nick {
            Some(nick) => extract_custom_nick(nick),
            None => "",
        },
    };

    let nick = format!("{} {} {}", rank.get_symbol(), name, custom_nick);
    let discord_member = discord_member.edit(&http, |e| e.nickname(nick)).await?;
    Ok(discord_member)
}
