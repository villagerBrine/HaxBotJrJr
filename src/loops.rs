//! Loops that listens to events and updates the bot / discord accordingly
use std::sync::Arc;

use serenity::http::Http;
use serenity::model::guild::{Guild, Member};
use serenity::CacheAndHttp;
use tokio::sync::RwLock;
use tracing::{info, instrument, warn};

use config::Config;
use event::{DiscordContext, DiscordEvent, DiscordSignal};
use memberdb::events::DBEvent;
use memberdb::model::discord::DiscordId;
use memberdb::model::member::{MemberId, MemberRank};
use memberdb::model::wynn::McId;
use memberdb::DB;
use util::discord;
use util::{ctxw, ok, some};
use wynn::events::{WynnEvent, WynnSignal};

/// Start event listening loops
pub async fn start_loops(
    cache_http: Arc<CacheAndHttp>, db: Arc<RwLock<DB>>, config: Arc<RwLock<Config>>, wynn_sig: WynnSignal,
    dc_sig: DiscordSignal,
) {
    let shared_cache_http = cache_http.clone();
    let shared_db = db.clone();
    let shared_config = config.clone();
    let shared_dc_sig = dc_sig.clone();
    tokio::spawn(async move {
        let guild = crate::wait_main_guild(shared_dc_sig).await;
        info!("Starting discord event listening loop (db event)");
        let mut recv = {
            let db = shared_db.read().await;
            db.connect()
        };
        loop {
            let event = recv.recv().await.unwrap();
            process_db_event(&shared_cache_http, &shared_db, &shared_config, &guild, &event).await;
        }
    });

    let shared_cache_http = cache_http.clone();
    let shared_db = db.clone();
    let shared_config = config.clone();
    let shared_dc_sig = dc_sig.clone();
    tokio::spawn(async move {
        let guild = crate::wait_main_guild(shared_dc_sig).await;
        info!("Starting discord event listening loop (wynn event)");
        let mut recv = wynn_sig.connect();
        loop {
            let events = recv.recv().await.unwrap();
            for event in events.as_ref() {
                process_wynn_event(&shared_cache_http, &shared_db, &shared_config, &guild, &event).await;
            }
        }
    });

    tokio::spawn(async move {
        info!("Starting discord event listening loop (discord event)");
        let mut recv = dc_sig.connect();
        loop {
            let event = recv.recv().await.unwrap();
            let (ctx, event) = event.as_ref();
            process_discord_event(&cache_http, &db, &config, &event, &ctx).await;
        }
    });
}

#[instrument(skip(cache_http, guild, db))]
async fn process_db_event(
    cache_http: &CacheAndHttp, db: &RwLock<DB>, config: &RwLock<Config>, guild: &Guild, event: &DBEvent,
) {
    match event {
        DBEvent::MemberAdd {
            discord_id: Some(discord_id), rank, mid, ..
        } => {
            let mut member = some!(get_discord_member(&cache_http, &guild, *discord_id).await, return);
            add_init_role_nick(&cache_http.http, &db, &config, *mid, *rank, &guild, &mut member).await;
        }
        DBEvent::MemberRemove { discord_id: Some(discord_id), .. }
        | DBEvent::DiscordProfileUnbind { before: discord_id, removed: false, .. } => {
            let mut member = some!(get_discord_member(&cache_http, &guild, *discord_id).await, return);
            remove_all_role_nick(&cache_http.http, &config, &guild, &mut member).await;
        }
        DBEvent::MemberRankChange { mid, new: rank, .. } => {
            let mut member = some!(get_discord_member_db(&cache_http, &db, *mid, &guild).await, return);

            if {
                let config = config.read().await;
                config.should_update_role(&member)
            } {
                info!("Updating discord roles");
                let _ = ctxw!(
                    crate::util::discord::fix_discord_roles(&cache_http.http, *rank, &guild, &mut member)
                        .await
                );
            }

            if {
                let config = config.read().await;
                config.should_update_nick(&member)
            } {
                info!("Updating discord nickname");
                if let Err(why) =
                    crate::util::discord::fix_member_nick(&cache_http.http, &db, *mid, &member, None).await
                {
                    warn!("Failed to update discord member's nickname: {:#}", why);
                }
            }
        }
        DBEvent::WynnProfileBind { mid, .. } | DBEvent::WynnProfileUnbind { mid, removed: false, .. } => {
            // Because discord nickname prioritize ign, so when a member's wynn profile is
            // binded / unbinded, their discord nickname need to be updated.
            let member = some!(get_discord_member_db(&cache_http, &db, *mid, &guild).await, return);

            {
                let config = config.read().await;
                if !config.should_update_nick(&member) {
                    return;
                }
            }

            info!("Updating discord nickname");
            if let Err(why) =
                crate::util::discord::fix_member_nick(&cache_http.http, &db, *mid, &member, None).await
            {
                warn!("Failed to update discord member's nickname: {:#}", why);
            }
        }
        DBEvent::DiscordProfileBind { mid, old, new } => {
            // Remove roles & nick from the old discord user
            if let Some(old_discord) = old {
                if let Some(mut old_member) = get_discord_member(&cache_http, &guild, *old_discord).await {
                    remove_all_role_nick(&cache_http.http, &config, &guild, &mut old_member).await;
                }
            }

            // Add roles & nick to the new discord user
            let mut member = some!(get_discord_member(&cache_http, &guild, *new).await, return);
            let rank = {
                let db = db.read().await;
                ok!(mid.rank(&mut db.exe()).await, return)
            };
            add_init_role_nick(&cache_http.http, &db, &config, *mid, rank, &guild, &mut member).await;
        }
        _ => {}
    }
}

#[instrument(skip(cache_http, guild, db))]
pub async fn process_wynn_event(
    cache_http: &CacheAndHttp, db: &RwLock<DB>, config: &RwLock<Config>, guild: &Guild, event: &WynnEvent,
) {
    match event {
        WynnEvent::MemberNameChange { id, new_name, .. } => {
            let mcid = McId(id.clone());
            // Update discord nick due to ign change.
            let (member, mid) = some!(get_discord_member_mc(&cache_http, &db, &mcid, &guild).await, return);

            {
                let config = config.read().await;
                if !config.should_update_nick(&member) {
                    return;
                }
            }

            let rank = {
                let db = db.read().await;
                ok!(mid.rank(&mut db.exe()).await, return)
            };
            let custom_nick = match &member.nick {
                Some(nick) => crate::util::discord::extract_custom_nick(nick),
                None => "",
            };
            let new_nick = format!("{} {} {}", rank.get_symbol(), new_name, custom_nick);
            let result = member.edit(&cache_http.http, |e| e.nickname(new_nick)).await;
            if let Err(why) = result {
                warn!("Failed to update discord member's nickname: {:#}", why);
            }
        }
        _ => {}
    }
}

#[instrument(skip(cache_http, db))]
pub async fn process_discord_event(
    cache_http: &CacheAndHttp, db: &RwLock<DB>, config: &RwLock<Config>, event: &DiscordEvent,
    ctx: &DiscordContext,
) {
    match event {
        DiscordEvent::MemberUpdate { old: Some(old), new, .. } => {
            // Update discord nick due to discord username change
            if old.user.name != new.user.name {
                {
                    let config = config.read().await;
                    if !config.should_update_nick(&new) {
                        return;
                    }
                }

                let id = ok!(DiscordId::try_from(new.user.id.0), return);
                let mid = {
                    let db = db.read().await;
                    some!(ok!(id.mid(&mut db.exe()).await, return), return)
                };
                // Only updates it if the discord nick is using discord username instead of ign
                let has_wynn = {
                    let db = db.read().await;
                    ok!(mid.links(&mut db.exe()).await, return).1.is_some()
                };
                if !has_wynn {
                    if let Err(why) =
                        crate::util::discord::fix_member_nick(&cache_http.http, &db, mid, new, None).await
                    {
                        warn!("Failed to update discord member's nickname: {:#}", why);
                    }
                }
            }
        }
        DiscordEvent::MemberJoin { member } => {
            if let Some(channel_id) = ctx.main_guild.system_channel_id {
                let msg = format!("{} Welcome to Hackforums! If you're interested in joining the guild, go to <#632705233864228881> and create a ticket!", member);
                if let Err(why) = channel_id.say(&cache_http.http, msg).await {
                    warn!("Failed to send welcome message: {:#}", why);
                }
            }
        }
        _ => {}
    }
}

/// Get a mcid's associated discord user and id.
pub async fn get_discord_member_mc(
    cache_http: &CacheAndHttp, db: &RwLock<DB>, mcid: &McId, guild: &Guild,
) -> Option<(Member, MemberId)> {
    let mid = {
        let db = db.read().await;
        match mcid.mid(&mut db.exe()).await {
            Ok(Some(mid)) => mid,
            _ => return None,
        }
    };
    let discord_id = {
        let db = db.read().await;
        ok!(mid.links(&mut db.exe()).await, return None).0
    };
    if let Some(discord_id) = discord_id {
        return match get_discord_member(&cache_http, &guild, discord_id).await {
            Some(member) => Some((member, mid)),
            None => None,
        };
    }
    None
}

/// Get a member id's associated discord member
pub async fn get_discord_member_db(
    cache_http: &CacheAndHttp, db: &RwLock<DB>, mid: MemberId, guild: &Guild,
) -> Option<Member> {
    let discord_id = {
        let db = db.read().await;
        ok!(mid.links(&mut db.exe()).await, return None).0
    };
    if let Some(discord_id) = discord_id {
        return get_discord_member(&cache_http, &guild, discord_id).await;
    }
    None
}

/// Get discord member via `DiscordId`
pub async fn get_discord_member(cache_http: &CacheAndHttp, guild: &Guild, id: DiscordId) -> Option<Member> {
    let user_id = ok!(u64::try_from(id.0), "Failed to convert DiscordId to UserId", return None);
    let member = ok!(guild.member(&cache_http, user_id).await, "Failed to get discord member", return None);
    Some(member)
}

/// Update a discord user's nick and add rank roles.
pub async fn add_init_role_nick(
    http: &Http, db: &RwLock<DB>, config: &RwLock<Config>, mid: MemberId, rank: MemberRank, guild: &Guild,
    member: &mut Member,
) {
    if {
        let config = config.read().await;
        config.should_update_nick(&member)
    } {
        info!("Updating discord roles");
        let _ = ctxw!(crate::util::discord::fix_discord_roles(&http, rank, &guild, member).await);
    }

    if {
        let config = config.read().await;
        config.should_update_role(&member)
    } {
        info!("Updating discord nickname");
        if let Err(why) = crate::util::discord::fix_member_nick(&http, &db, mid, member, Some("")).await {
            warn!("Failed to set discord member's initial nickname: {:#}", why);
        }
    }
}

/// Remove a discord user's nick and all rank roles.
pub async fn remove_all_role_nick(http: &Http, config: &RwLock<Config>, guild: &Guild, member: &mut Member) {
    if {
        let config = config.read().await;
        config.should_update_role(&member)
    } {
        info!("Removing discord rank roles");
        for rank in memberdb::model::member::MANAGED_MEMBER_RANKS {
            let _ = ctxw!(discord::remove_role_maybe(&http, rank.get_role(&guild), member).await);
            let _ = ctxw!(discord::remove_role_maybe(&http, rank.get_group_role(&guild), member).await);
        }
    }

    if {
        let config = config.read().await;
        config.should_update_nick(&member)
    } {
        info!("Removing discord nickname");
        let result = member.edit(&http, |e| e.nickname("")).await;
        if let Err(why) = result {
            warn!("Failed to remove discord member's nickname: {:#}", why);
        }
    }
}
