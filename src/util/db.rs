//! Utility function for interacting with `memberdb`
use std::borrow::Cow;

use reqwest::Client;
use serenity::client::Context;
use serenity::model::channel::Message;
use serenity::model::guild::{Guild, Member};
use serenity::model::id::UserId;
use tokio::sync::RwLock;

use memberdb::model::member::MemberId;
use memberdb::DB;
use msgtool::parser::{DiscordObject, TargetObject};
use util::{ctx, ok, ok_some, some};

use crate::util::Terminator::{self, *};
use crate::{t, tfinish, ttry};

/// Given discord id and mc id, return their linked member ids.
pub async fn get_profile_mids(db: &RwLock<DB>, discord_id: i64, mcid: &str) -> (Option<i64>, Option<i64>) {
    let db = db.read().await;
    let mid1 = ok!(ctx!(memberdb::get_wynn_mid(&db, mcid).await, "Failed to get wynn mid"), None);
    let mid2 = ok!(ctx!(memberdb::get_discord_mid(&db, discord_id).await, "Failed to get discord mid"), None);
    (mid1, mid2)
}

/// Get discord id and mc id via discord name and ign.
/// A discord member is also returned in case if you needs it.
pub async fn get_profile_ids<'a>(
    ctx: &'a Context, msg: &Message, guild: &'a Guild, client: &Client, discord_name: &'a str, ign: &str,
) -> Terminator<(Cow<'a, Member>, i64, String)> {
    let discord_member = some!(
        ttry!(util::discord::get_member_named(&ctx.http, guild, discord_name).await),
        tfinish!(ctx, msg, "Failed to find an discord user with the given name")
    );
    let discord_id =
        ttry!(i64::try_from(discord_member.as_ref().user.id.0), "Failed to convert u64 into i64");

    let mcid = ok!(wynn::get_id(client, ign).await, tfinish!(ctx, msg, "Provided mc ign doesn't exist"));

    Proceed((discord_member, discord_id, mcid))
}

/// Parse a target expression into `TargetId`
pub async fn parse_user_target(
    ctx: &Context, msg: &Message, db: &RwLock<DB>, client: &Client, guild: &Guild, s: &str,
) -> Terminator<TargetId> {
    let target = match TargetObject::from_str(ctx, db, client, guild, s).await {
        Ok(v) => v,
        Err(why) => tfinish!(ctx, msg, format!("invalid target: {}", why)),
    };
    Proceed(match target {
        TargetObject::Discord(discord_obj) => match *discord_obj {
            DiscordObject::Member(member) => TargetId::Discord(member.as_ref().user.id),
            _ => tfinish!(ctx, msg, "Only discord/mc user are accepted as target"),
        },
        TargetObject::Mc(id) => TargetId::Wynn(id),
    })
}

/// Parse a target expression into member id
pub async fn parse_user_target_mid(
    ctx: &Context, msg: &Message, db: &RwLock<DB>, client: &Client, guild: &Guild, s: &str,
) -> Terminator<MemberId> {
    let target = t!(?parse_user_target(ctx, msg, db, client, guild, s).await);
    Proceed(some!(
        {
            let db = db.read().await;
            target.get_mid(&db).await
        },
        tfinish!(ctx, msg, "Failed to find target member in database")
    ))
}

#[derive(Debug)]
/// Discord user id or mcid
pub enum TargetId {
    Discord(UserId),
    Wynn(String),
}

impl TargetId {
    /// Get linked member id
    pub async fn get_mid(&self, db: &DB) -> Option<MemberId> {
        match self {
            Self::Discord(id) => {
                let id = ok!(i64::try_from(*id), return None);
                ok_some!(memberdb::get_discord_mid(&db, id).await)
            }
            Self::Wynn(id) => ok_some!(memberdb::get_wynn_mid(&db, &id).await),
        }
    }
}
