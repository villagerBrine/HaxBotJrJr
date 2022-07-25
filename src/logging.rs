use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use serenity::CacheAndHttp;
use tokio::sync::RwLock;
use tokio::time::{self, Duration};
use tracing::info;

use config::Config;
use event::{WynnEvent, WynnSignal};
use util::{ctx, ok, some};

fn make_wynn_log(event: &WynnEvent) -> Option<String> {
    Some(match event {
        WynnEvent::MemberJoin { ign, .. } => format!("**{}** joined the guild", ign),
        WynnEvent::MemberLeave { ign, rank, .. } => format!("**{}** ({}) left the guild", ign, rank),
        WynnEvent::MemberRankChange { ign, old_rank, new_rank, .. } => {
            format!("**{}** guild rank changed, from __{}__ to __{}__", ign, old_rank, new_rank)
        }
        WynnEvent::MemberContribute { ign, old_contrib, new_contrib, .. } => {
            let delta = util::string::fmt_num(new_contrib - old_contrib, false);
            let new_contrib = util::string::fmt_num(*new_contrib, true);
            format!("**{}** contributed __{}__ xp, total *{}* xp", ign, delta, new_contrib)
        }
        WynnEvent::MemberNameChange { old_name, new_name, .. } => {
            format!("**{}** changed their name to **{}**", old_name, new_name)
        }
        WynnEvent::GuildLevelUp { level } => format!("**Guild leveled up to** __{}__", level),
        WynnEvent::PlayerJoin { ign, world } => format!("**{}** joined __WC{}__", ign, world),
        WynnEvent::PlayerLeave { ign } => format!("**{}** logged off", ign),
        _ => return None,
    })
}

pub const LOG_CHANNEL_KEYS: [&str; 4] = ["guild_member", "guild_level", "xp", "online"];

fn get_log_channel_key(event: &WynnEvent) -> Option<&str> {
    Some(match event {
        WynnEvent::MemberJoin { .. }
        | WynnEvent::MemberLeave { .. }
        | WynnEvent::MemberRankChange { .. }
        | WynnEvent::MemberNameChange { .. } => "guild_member",
        WynnEvent::GuildLevelUp { .. } => "guild_level",
        WynnEvent::MemberContribute { .. } => "xp",
        WynnEvent::PlayerJoin { .. } | WynnEvent::PlayerLeave { .. } => "online",
        _ => return None,
    })
}

pub async fn start_loop(cache_http: Arc<CacheAndHttp>, config: Arc<RwLock<Config>>, signal: WynnSignal) {
    {
        // Makes sure the log channel map entries exists
        let mut config = config.write().await;
        for k in LOG_CHANNEL_KEYS {
            if !config.log_channel_map.contains_key(k) {
                config.log_channel_map.insert(k.to_string(), HashSet::new());
            }
        }
    }

    let mut buffers: HashMap<&str, String> = HashMap::with_capacity(LOG_CHANNEL_KEYS.len());
    for k in LOG_CHANNEL_KEYS {
        buffers.insert(k, String::new());
    }
    let buffers = Arc::new(Mutex::new(buffers));

    let shared_buffers = Arc::clone(&buffers);
    let shared_config = Arc::clone(&config);
    tokio::spawn(async move {
        info!("Starting discord log channel loop");
        let mut interval = time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            for key in LOG_CHANNEL_KEYS {
                let log = {
                    let mut buffers = shared_buffers.lock().unwrap();
                    // Early return if no logs
                    let buffer = buffers.get(key).unwrap();
                    if buffer.is_empty() {
                        continue;
                    }
                    // Get buffer and replace it with an empty one
                    let buffer = buffers.remove(key).unwrap();
                    buffers.insert(key, String::new());
                    buffer
                };
                {
                    let config = shared_config.read().await;
                    let _ = ctx!(config.send_log(&cache_http, key, &log).await);
                }
            }
        }
    });

    tokio::spawn(async move {
        info!("Starting wynn event logging loop");
        let mut receiver = signal.connect();
        loop {
            let events =
                ok!(ctx!(receiver.recv().await, "Failed to receive wynn events in log loop"), continue);

            for event in events.as_ref() {
                let key = some!(get_log_channel_key(&event), continue);
                // Do not log if there are no log channels
                let is_channel_empty = {
                    let config = config.read().await;
                    config.log_channel_map.get(key).unwrap().is_empty()
                };
                if is_channel_empty {
                    continue;
                }

                let log = some!(make_wynn_log(&event), continue);
                {
                    let mut buffers = buffers.lock().unwrap();
                    let buffer = buffers.get_mut(key).unwrap();
                    buffer.push('\n');
                    buffer.push_str(&log);
                }
            }
        }
    });
}
