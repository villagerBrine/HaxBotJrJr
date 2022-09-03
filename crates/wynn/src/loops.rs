use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;
use std::time::Duration as StdDuration;

use anyhow::Result;
use reqwest::Client;
use serenity::async_trait;
use tokio::time::{self, Duration};
use tracing::{error, info};

use util::ok;

use crate::cache::Cache;
use crate::events::{WynnEvent, WynnSignal};
use crate::model::{Guild, GuildMember, ServerList};

/// Start loops for fetching and analyzing of wynncraft api and broadcasting [`WynnEvent`]
///
/// This function need to be called for [`Cache`] and [`WynnEvent`] to work.
///
/// [`WynnEvent`]: event::WynnEvent
/// [`Cache`]: crate::cache::Cache
pub async fn start_loops(
    signal: WynnSignal, client: Client, cache: Arc<Cache>, tracked_ign: impl TrackedIgn,
) {
    let shared_signal = signal.clone();
    let shared_client = client.clone();
    let shared_cache = Arc::clone(&cache);
    tokio::spawn(async move {
        // This is to make sure wynn events are sent after receivers are created
        thread::sleep(StdDuration::from_secs(5));
        main_guild_api_loop(shared_signal, &shared_client, &shared_cache).await;
    });

    tokio::spawn(async move {
        server_api_loop(signal, &client, tracked_ign, &cache).await;
    });
}

/// Trait for getting set of tracked igns.
#[async_trait]
pub trait TrackedIgn: Send + Sync + 'static {
    /// Get set of tracked igns for online status tracking.
    async fn tracked_ign(&self) -> Result<HashSet<String>>;
}

/// Starts a loop to analyze main guild statistics and broadcast [`WynnEvent`]
///
/// [`WynnEvent`]: event::WynnEvent
async fn main_guild_api_loop(signal: WynnSignal, client: &Client, cache: &Cache) {
    let mut interval = time::interval(Duration::from_millis(10000));
    let mut prev_timestamp = 0;

    let mut url = "https://api.wynncraft.com/public_api.php?action=guildStats&command=".to_string();
    url.push_str(&std::env::var("GUILD_NAME").expect("Expected guild name in environment"));

    info!("Starting main guild loop");
    loop {
        interval.tick().await;

        let resp = client.get(&url).send().await;
        if let Err(why) = &resp {
            crate::utils::request_error_log(why, "main guild stats");
            continue;
        }
        let resp = resp.unwrap().json::<Guild>().await;
        if let Err(why) = resp {
            error!("Failed to parse responce to json when requesting main guild stats: {}", why);
            continue;
        }
        let mut resp = resp.unwrap();

        // Checks if the response is outdated
        if resp.request.timestamp > prev_timestamp {
            prev_timestamp = resp.request.timestamp;
        } else {
            continue;
        }

        // Creates a map from mcid to guild member statistics for easy access
        let resp_map = make_member_map(&mut resp.members);

        let mut events: Vec<WynnEvent> = Vec::new();
        {
            let cache_resp = cache.guild.read().await;
            match cache_resp.as_ref() {
                Some(cache_resp) => {
                    // Checking for guild global events
                    if cache_resp.level < resp.level {
                        info!(level = resp.level, "Guild level up");
                        events.push(WynnEvent::GuildLevelUp { level: resp.level });
                    }
                }
                None => {
                    // This is needed so database can be populated during the bot's initial run
                    info!("Emitting MemberJoin events for all members due to empty guild cache");
                    for member in resp_map.values() {
                        events.push(WynnEvent::MemberJoin {
                            id: member.uuid.clone(),
                            rank: member.rank.clone(),
                            ign: member.name.clone(),
                            xp: member.contributed,
                        })
                    }
                }
            }

            let cache_map = cache.members.read().await;
            if let Some(cache_map) = cache_map.as_ref() {
                // Checks for new member by comparing the current member map with the cached one
                for member in resp_map.values() {
                    if !cache_map.contains_key(&member.uuid) {
                        info!(%member.name, "Guild member join");

                        events.push(WynnEvent::MemberJoin {
                            id: member.uuid.clone(),
                            rank: member.rank.clone(),
                            ign: member.name.clone(),
                            xp: member.contributed,
                        });
                    }
                }
                for member in cache_map.values() {
                    match resp_map.get(&member.uuid) {
                        // Checks for changes in member statistics
                        Some(resp_member) => events.append(&mut get_member_events(member, resp_member)),
                        // Missing member, add `MemberLeave` event
                        None => {
                            info!(%member.name, "Guild member leave");
                            events.push(WynnEvent::MemberLeave {
                                id: member.uuid.clone(),
                                rank: member.rank.clone(),
                                ign: member.name.clone(),
                            });
                        }
                    }
                }
            }
        }

        // Emit events and update caches
        signal.signal(events);

        {
            let mut cache = cache.guild.write().await;
            *cache = Some(resp);
        }

        {
            let mut cache = cache.members.write().await;
            *cache = Some(resp_map);
        }
    }
}

/// Analyzes old & new guild member statistics for [`WynnEvent`]
///
/// [`WynnEvent`]: event::WynnEvent
fn get_member_events(old: &GuildMember, new: &GuildMember) -> Vec<WynnEvent> {
    let mut events = Vec::new();

    // checks for name change
    if old.name != new.name {
        info!(%old.name, %new.name, "Guild member name change");
        events.push(WynnEvent::MemberNameChange {
            id: old.uuid.clone(),
            old_name: old.name.clone(),
            new_name: new.name.clone(),
        })
    }

    // checks for rank change
    if old.rank != new.rank {
        info!(old.rank, new.rank, "Guild member rank change");
        events.push(WynnEvent::MemberRankChange {
            id: old.uuid.clone(),
            ign: new.name.clone(),
            old_rank: old.rank.clone(),
            new_rank: new.rank.clone(),
        });
    }

    // checks for contribution change
    if old.contributed < new.contributed {
        info!(name=%new.name, old.contributed, new.contributed, "Guild member contribution");

        events.push(WynnEvent::MemberContribute {
            id: old.uuid.clone(),
            ign: new.name.clone(),
            old_contrib: old.contributed,
            new_contrib: new.contributed,
        })
    }

    events
}

/// Starts a loop to analyze server online players and broadcast [`WynnEvent`]
///
/// [`WynnEvent`]: event::WynnEvent
async fn server_api_loop(signal: WynnSignal, client: &Client, tracked_ign: impl TrackedIgn, cache: &Cache) {
    let mut interval = time::interval(Duration::from_secs(60));
    let mut prev_timestamp: u64 = 0;
    let mut first_loop = true;

    let url = "https://api.wynncraft.com/public_api.php?action=onlinePlayers";

    info!("Starting server list loop");
    loop {
        interval.tick().await;

        let resp = client.get(url).send().await;
        if let Err(why) = &resp {
            crate::utils::request_error_log(why, "server list");
            continue;
        }
        let resp = resp.unwrap().json::<ServerList>().await;
        if let Err(why) = resp {
            error!("Failed to parse responce to json when requesting server list: {}", why);
            continue;
        }
        let mut resp = resp.unwrap();

        let mut found_meta = false;
        let mut elapsed = 0;
        // Navigating to the response's timestamp field
        if let Some(req_meta) = resp.remove("request") {
            if let Some(req_meta) = req_meta.as_object() {
                if let Some(timestamp) = req_meta.get("timestamp") {
                    if let Some(timestamp) = timestamp.as_u64() {
                        // Checks if the response is outdated
                        if timestamp > prev_timestamp {
                            // Get the elapsed time between it and previous response
                            if prev_timestamp != 0 {
                                elapsed = timestamp - prev_timestamp;
                            }
                            prev_timestamp = timestamp;
                            found_meta = true;
                        } else {
                            continue;
                        }
                    }
                }
            }
        }

        if !found_meta {
            error!("Failed to get server list response metadata");
            continue;
        }

        let mut events: Vec<WynnEvent> = Vec::new();
        // Getting all track-able igns from database
        let mut all_igns: HashSet<String> =
            ok!(tracked_ign.tracked_ign().await, "Failed to get tracked igns", continue);

        if first_loop {
            // initialize `tracked_ign`
            let mut tracked_ign = cache.online.write().await;
            for (world, igns) in iter_ign(&resp) {
                for ign in igns {
                    if all_igns.contains(ign) {
                        tracked_ign.insert(world.clone(), ign.to_string());
                    }
                }
            }
            first_loop = false;
            continue;
        }

        {
            let mut tracked_ign = cache.online.write().await;
            for (world, igns) in iter_ign(&resp) {
                for ign in igns {
                    // Filter out igns that can't be tracked, aka not in database
                    if all_igns.contains(ign) {
                        match tracked_ign.world(ign) {
                            Some(old_world) => {
                                // Player was online previously
                                // Update online time as the elapsed time from previous loop
                                if elapsed > 0 {
                                    events.push(WynnEvent::PlayerStay {
                                        ign: ign.to_string(),
                                        world: world.clone(),
                                        elapsed,
                                    });
                                }
                                if world != old_world {
                                    info!(ign, old_world, world, "player move");
                                    events.push(WynnEvent::PlayerMove {
                                        ign: ign.to_string(),
                                        old_world: old_world.to_string(),
                                        new_world: world.clone(),
                                    });
                                }
                            }
                            None => {
                                // Player just logged on as they aren't online during previous loop
                                // Add to tracked ign
                                info!(ign, world, "player join");
                                tracked_ign.insert(world.clone(), ign.to_string());
                                events.push(WynnEvent::PlayerJoin {
                                    ign: ign.to_string(),
                                    world: world.clone(),
                                });
                            }
                        }
                        // Remove online ign from it, so all it lefts are track-able igns that
                        // are offline
                        all_igns.remove(ign);
                    }
                }
            }

            // `all_igns` now contains all offline igns
            let mut empty_worlds = Vec::new();
            for ign in all_igns {
                if let Some((world, is_empty)) = tracked_ign.remove(&ign) {
                    info!(ign, "player leave");
                    if is_empty {
                        empty_worlds.push(world.to_string());
                    }
                    events.push(WynnEvent::PlayerLeave { ign, world: world.to_string() });
                }
            }

            // Remove empty worlds
            for world in &empty_worlds {
                tracked_ign.0.remove(world);
            }
        }

        signal.signal(events);
    }
}

/// Help function for constructing a map from server to its online players, excluding lobby
/// servers.
fn iter_ign<'a>(resp: &'a ServerList) -> HashMap<&'_ String, impl Iterator<Item = &'_ str>> {
    let mut ign_map = HashMap::new();
    for (world, players) in resp.iter() {
        if !world.starts_with("WC") {
            continue;
        }

        if let Some(players) = players.as_array() {
            let iter = players.iter().filter_map(|ign| ign.as_str());
            ign_map.insert(world, iter);
        }
    }
    ign_map
}

/// Help function for constructing a map from mcid to guild member stats.
fn make_member_map(member: &mut Vec<GuildMember>) -> HashMap<String, GuildMember> {
    let mut map = HashMap::with_capacity(member.len());
    while let Some(m) = member.pop() {
        map.insert(m.uuid.clone(), m);
    }
    map
}
