use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use serenity::client::Cache;
use serenity::http::CacheHttp;
use serenity::model::channel::Channel;
use serenity::model::id::ChannelId;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{self, Duration as ADuration};
use tracing::{error, info, instrument};

use config::Config;
use event::timer::{TimerEvent, TimerSignal};
use event::{DiscordContext, DiscordEvent, DiscordSignal, WynnEvent, WynnSignal};
use util::{ctx, ok, some};

use crate::guild::GuildRank;
use crate::voice_tracker::VoiceTracker;
use crate::DB;

pub async fn start_loops(
    db: Arc<RwLock<DB>>, config: Arc<RwLock<Config>>, cache: Arc<Cache>, vt: Arc<Mutex<VoiceTracker>>,
    wynn_sig: WynnSignal, dc_sig: DiscordSignal, timer_sig: TimerSignal,
) {
    let shared_db = db.clone();
    tokio::spawn(async move {
        info!("Starting member manage loop (wynn event)");
        let mut recv = wynn_sig.connect();
        loop {
            let events = recv.recv().await.unwrap();
            let mut events_to_send = Vec::new();

            for event in events.as_ref() {
                if let Some(ref mut events) = process_wynn_event(&shared_db, &event).await {
                    events_to_send.append(events);
                }
            }

            if !events_to_send.is_empty() {
                wynn_sig.signal(events_to_send);
            }
        }
    });

    let shared_db = db.clone();
    let shared_vt = vt.clone();
    tokio::spawn(async move {
        info!("Starting member manage loop (discord event)");
        let mut recv = dc_sig.connect();
        loop {
            let event = recv.recv().await.unwrap();
            let (ctx, event) = event.as_ref();
            process_discord_event(&shared_db, &config, &shared_vt, event, ctx).await;
        }
    });

    let shared_db = db.clone();
    tokio::spawn(async move {
        info!("Starting voice tracking update loop");
        let mut interval = time::interval(ADuration::from_secs(60));
        loop {
            interval.tick().await;
            let mut vt = vt.lock().await;
            for (id, dur) in vt.track_all_voice() {
                track_voice_db(&shared_db, *id, dur).await;
            }
        }
    });

    tokio::spawn(async move {
        info!("Starting member manage loop (timer event)");
        let mut recv = timer_sig.connect();
        loop {
            let event = recv.recv().await.unwrap();
            match event.as_ref() {
                TimerEvent::Weekly => {
                    info!("Starting weekly reset");
                    let db = db.write().await;
                    let _ = ctx!(crate::weekly_reset(&db, &cache).await, "Failed weekly reset");
                }
            }
        }
    });
}

#[instrument(skip(db))]
pub async fn process_wynn_event(db: &RwLock<DB>, event: &WynnEvent) -> Option<Vec<WynnEvent>> {
    match event {
        WynnEvent::MemberJoin { id, rank, ign, xp } => {
            let mid = {
                let db = db.read().await;
                ok!(crate::get_wynn_mid(&db, id).await, "Failed to get wynn.mid", return None)
            };

            match mid {
                Some(_) => {
                    // Checking for changes
                    let (old_ign, old_rank) = {
                        let db = db.read().await;
                        let old_ign = crate::get_ign(&db, id).await;
                        let old_rank = crate::get_guild_rank(&db, id).await;
                        (old_ign, old_rank)
                    };

                    let mut events = Vec::new();

                    if let Ok(old_ign) = old_ign {
                        if old_ign != *ign {
                            info!(%id, %old_ign, %ign, "Found ign change");
                            events.push(WynnEvent::MemberNameChange {
                                id: id.to_string(),
                                old_name: old_ign,
                                new_name: ign.to_string(),
                            });
                        }
                    }

                    if let Ok(old_rank) = old_rank {
                        if old_rank.to_api() != *rank {
                            info!(%id, %old_rank, %rank, "Found guild rank change");
                            events.push(WynnEvent::MemberRankChange {
                                id: id.to_string(),
                                ign: ign.to_string(),
                                old_rank: old_rank.to_string(),
                                new_rank: rank.to_string(),
                            });
                        }
                    }

                    return Some(events);
                }
                None => {
                    info!(%id, %rank, %ign, "Adding guild member");
                    let rank = ok!(GuildRank::from_api(rank), "Error", return None);
                    let db = db.write().await;
                    ok!(
                        crate::bind_wynn_guild(&db, id, ign, true, rank).await,
                        "Failed to add guild member",
                        return None
                    );
                    info!(%id, xp, "Initialize new guild member's xp");
                    ok!(crate::update_xp(&db, id, *xp).await, "Failed to initalize xp", return None)
                }
            }
        }
        WynnEvent::MemberLeave { id, rank, ign } => {
            info!(%id, %rank, %ign, "Removing guild member");
            let rank = ok!(GuildRank::from_api(rank), "Error", return None);
            let db = db.write().await;
            ok!(
                crate::bind_wynn_guild(&db, id, ign, false, rank).await,
                "Failed to remove guild member",
                return None
            );
        }
        WynnEvent::MemberRankChange { id, old_rank, new_rank, ign } => {
            info!(ign, %old_rank, %new_rank, "Updating guild member guild rank");
            let rank = ok!(GuildRank::from_api(new_rank), "Error", return None);
            let db = db.write().await;
            ok!(
                crate::update_guild_rank(&db, id, rank).await,
                "Failed to update guild member guild rank",
                return None
            )
        }
        WynnEvent::MemberNameChange { id, new_name, .. } => {
            info!(%id, %new_name, "Updating guild member ign");
            let db = db.write().await;
            ok!(crate::update_ign(&db, id, new_name).await, "Failed to update guild member ign", return None)
        }
        WynnEvent::MemberContribute { id, old_contrib, new_contrib, ign } => {
            let amount = new_contrib - old_contrib;
            info!(ign, amount, "Updating guild member xp");
            let db = db.write().await;
            ok!(crate::update_xp(&db, id, amount).await, "Failed to increment guild member xp", return None)
        }
        WynnEvent::PlayerStay { ign, world: _world, elapsed } => {
            let id = {
                let db = db.read().await;
                ok!(crate::get_ign_mcid(&db, ign).await, "Failed to get id of ign from db", return None)
            };
            if let Some(id) = id {
                let db = db.write().await;
                let elapsed = ok!(
                    i64::try_from(*elapsed),
                    "Failed to convert elapsed activity: u64 to i64",
                    return None
                );
                ok!(
                    crate::update_activity(&db, &id, elapsed).await,
                    "Failed to update wynn activity",
                    return None
                );
            }
        }
        _ => {}
    }
    None
}

pub async fn process_discord_event(
    db: &RwLock<DB>, config: &RwLock<Config>, vt: &Mutex<VoiceTracker>, event: &DiscordEvent,
    ctx: &DiscordContext,
) {
    match event {
        DiscordEvent::Message { message } => {
            let channel = ok!(message.channel(&ctx).await, "Failed to get message's guild", return);
            let channel = match channel {
                Channel::Guild(c) => c,
                _ => return,
            };
            {
                let config = config.read().await;
                if !config.is_channel_tracked(&ctx.cache, &channel) {
                    return;
                }
            }

            let id = some!(
                crate::utils::from_user_id(message.author.id),
                "Failed to convert UserId to DiscordId",
                return
            );
            let mid = {
                let db = db.read().await;
                ok!(crate::get_discord_mid(&db, id).await, return)
            };
            if mid.is_some() {
                let db = db.write().await;
                ok!(crate::update_message(&db, 1, id).await, "Failed to update discord message stat", return);
            }
        }
        DiscordEvent::VoiceJoin { state } => {
            if !ok!(is_channel_id_tracked(&ctx, &config, some!(state.channel_id, return)).await, return) {
                return;
            }

            {
                let db = db.read().await;
                if !crate::utils::is_discord_member(&db, &state.user_id).await {
                    return;
                }
            }

            if !state.mute && !state.deaf {
                info!(id = state.user_id.0, "Begin tracking for user joined voice chat");
                let mut vt = vt.lock().await;
                vt.track_voice(&state.user_id.0);
            }
        }
        DiscordEvent::VoiceLeave { old_state } => {
            if !ok!(is_channel_id_tracked(&ctx, &config, some!(old_state.channel_id, return)).await, return) {
                return;
            }

            {
                let db = db.read().await;
                if !crate::utils::is_discord_member(&db, &old_state.user_id).await {
                    return;
                }
            }

            if old_state.mute || old_state.deaf {
                return;
            }

            info!(id = old_state.user_id.0, "Finish tracking for user left voice chat");
            let dur = {
                let mut vt = vt.lock().await;
                some!(vt.untrack_voice(&old_state.user_id.0), return)
            };

            track_voice_db(&db, old_state.user_id.0, dur).await;
        }
        DiscordEvent::VoiceChange { old_state, new_state } => {
            let old_tracked =
                !ok!(is_channel_id_tracked(&ctx, &config, some!(old_state.channel_id, return)).await, return);
            if old_state.channel_id == new_state.channel_id && !old_tracked {
                return;
            }
            let new_tracked =
                !ok!(is_channel_id_tracked(&ctx, &config, some!(new_state.channel_id, return)).await, return);

            if !old_tracked && !new_tracked {
                return;
            }

            {
                let db = db.read().await;
                if !crate::utils::is_discord_member(&db, &new_state.user_id).await {
                    return;
                }
            }

            let old_active = !old_state.mute && !old_state.deaf && old_tracked;
            let new_active = !new_state.mute && !new_state.deaf && new_tracked;

            if old_active && !new_active {
                info!(id = old_state.user_id.0, "Finish tracking for user no longer valid for tracking");
                let dur = {
                    let mut vt = vt.lock().await;
                    some!(vt.untrack_voice(&new_state.user_id.0), return)
                };
                track_voice_db(&db, new_state.user_id.0, dur).await;
            } else if !old_active && new_active {
                info!(id = old_state.user_id.0, "Begin tracking for user became valid for tracking");
                let mut vt = vt.lock().await;
                vt.track_voice(&new_state.user_id.0);
            }
        }
        _ => {}
    }
}

async fn track_voice_db(db: &RwLock<DB>, user_id: u64, dur: Duration) {
    let dur = ok!(i64::try_from(dur.as_secs()), "Failed to convert u64 to i64 (duration)", return);
    let discord_id = ok!(i64::try_from(user_id), "Failed to convert u64 to i64 (id)", return);

    let db = db.read().await;
    if let Err(why) = crate::update_voice(&db, dur, discord_id).await {
        error!("Failed to update voice chat activity stat: {:#}", why);
    }
}

async fn is_channel_id_tracked(
    cache_http: &impl CacheHttp, config: &RwLock<Config>, channel_id: ChannelId,
) -> Result<bool> {
    let channel = channel_id.to_channel(&cache_http).await.context("Failed to get channel data")?;
    if let Channel::Guild(channel) = channel {
        let config = config.read().await;
        match cache_http.cache() {
            Some(cache) => {
                if config.is_channel_tracked(&cache, &channel) {
                    return Ok(true);
                }
            }
            None => error!("Failed to get cache"),
        }
    }
    Ok(false)
}
