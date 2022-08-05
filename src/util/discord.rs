use anyhow::Result;
use serenity::http::Http;
use serenity::model::guild::{Guild, Member};
use tokio::sync::RwLock;

use memberdb::model::member::{MemberId, MemberRank};
use memberdb::DB;
use msgtool::pager::ToPage;
use util::ctx;

/// Update discord member's role to match up with their rank
pub async fn fix_discord_roles(
    http: &Http, rank: MemberRank, guild: &Guild, member: &mut Member,
) -> Result<()> {
    if memberdb::model::member::MANAGED_MEMBER_RANKS.contains(&rank) {
        util::discord::add_role(&http, rank.get_role(&guild), member).await?;
        util::discord::add_role(&http, rank.get_group_role(&guild), member).await?;
    }

    for other_rank in memberdb::model::member::MANAGED_MEMBER_RANKS {
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
            Some(nick_i) => &nick[no_rank_i + nick_i + 2..],
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

/// A 2d vector that can be formatted into a minimal lb table via `ToPage`
pub struct MinimalLB(pub Vec<Vec<String>>);

impl ToPage for MinimalLB {
    type Page = String;

    fn to_page(&self, page_info: Option<(usize, usize)>) -> Self::Page {
        make_minimal_table(&self.0, page_info, |row_s, i, s| match i {
            0 => row_s.push_str(s),
            1 => {
                row_s.push_str(" **");
                row_s.push_str(s);
                row_s.push_str("** ");
            }
            _ => push_empty_or(row_s, s),
        })
    }
}

/// A 2d vector that can be formatted into a minimal member list via `ToPage`
pub struct MinimalMembers(pub Vec<Vec<String>>);

impl ToPage for MinimalMembers {
    type Page = String;

    fn to_page(&self, page_info: Option<(usize, usize)>) -> Self::Page {
        make_minimal_table(&self.0, page_info, |row_s, i, s| match i {
            0 => push_empty_or(row_s, s),
            1 => push_empty_or(row_s, s),
            _ => {
                row_s.push_str("**");
                row_s.push_str(s);
                row_s.push_str("**");
            }
        })
    }
}

/// Push string, if it is empty, push "none"
fn push_empty_or(row_s: &mut String, s: &String) {
    if s.is_empty() {
        row_s.push_str("none ")
    } else {
        row_s.push('`');
        row_s.push_str(s);
        row_s.push_str("` ");
    }
}

/// Format a 2d vector into a minimal table
fn make_minimal_table<F>(data: &Vec<Vec<String>>, page_info: Option<(usize, usize)>, fmt: F) -> String
where
    F: Fn(&mut String, usize, &String) -> (),
{
    let mut page = data
        .iter()
        .map(|row| {
            let mut row_s = String::new();
            for (i, s) in row.iter().enumerate() {
                fmt(&mut row_s, i, s);
            }
            row_s
        })
        .collect::<Vec<String>>()
        .join("\n");

    match page_info {
        Some((_, 1)) | None => {}
        Some((index, num)) => {
            page.push_str("\n__");
            page.push_str(&index.to_string());
            page.push('/');
            page.push_str(&num.to_string());
            page.push_str("__");
        }
    }

    page
}
