use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use chrono::offset::Utc;
use serenity::CacheAndHttp;
use tokio::sync::RwLock;
use tokio::time::{self, Duration};
use tracing::info;

use config::tag::TextChannelTag;
use config::Config;
use event::{WynnEvent, WynnSignal};
use memberdb::events::DBEvent;
use memberdb::DB;
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
        WynnEvent::PlayerJoin { ign, world } => format!("**{}** logged in at __{}__", ign, world),
        WynnEvent::PlayerLeave { ign } => format!("**{}** logged off", ign),
        _ => return None,
    })
}

pub const LOG_CHANNEL_TAGS: [TextChannelTag; 4] = [
    TextChannelTag::GuildMemberLog,
    TextChannelTag::GuildLevelLog,
    TextChannelTag::XpLog,
    TextChannelTag::OnlineLog,
];

fn get_log_channel_tag(event: &WynnEvent) -> Option<TextChannelTag> {
    Some(match event {
        WynnEvent::MemberJoin { .. }
        | WynnEvent::MemberLeave { .. }
        | WynnEvent::MemberRankChange { .. }
        | WynnEvent::MemberNameChange { .. } => TextChannelTag::GuildMemberLog,
        WynnEvent::GuildLevelUp { .. } => TextChannelTag::GuildLevelLog,
        WynnEvent::MemberContribute { .. } => TextChannelTag::XpLog,
        WynnEvent::PlayerJoin { .. } | WynnEvent::PlayerLeave { .. } => TextChannelTag::OnlineLog,
        _ => return None,
    })
}

pub async fn start_log_loop(cache_http: Arc<CacheAndHttp>, config: Arc<RwLock<Config>>, signal: WynnSignal) {
    let mut buffers: HashMap<&TextChannelTag, String> = HashMap::with_capacity(LOG_CHANNEL_TAGS.len());
    for k in &LOG_CHANNEL_TAGS {
        buffers.insert(k, String::new());
    }
    let buffers = Arc::new(Mutex::new(buffers));
    let xp_buffer: HashMap<String, (String, i64, i64)> = HashMap::new();
    let xp_buffer = Arc::new(Mutex::new(xp_buffer));

    let shared_buffers = Arc::clone(&buffers);
    let shared_xp_buffer = Arc::clone(&xp_buffer);
    let shared_config = Arc::clone(&config);
    tokio::spawn(async move {
        info!("Starting discord log channel loop");
        let mut interval = time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            for tag in &LOG_CHANNEL_TAGS {
                let log = {
                    let mut buffers = shared_buffers.lock().unwrap();
                    // Early return if no logs
                    let buffer = buffers.get(tag).unwrap();
                    if buffer.is_empty() {
                        continue;
                    }
                    // Get buffer and replace it with an empty one
                    let buffer = buffers.remove(tag).unwrap();
                    buffers.insert(tag, String::new());
                    buffer
                };
                {
                    let config = shared_config.read().await;
                    let _ = ctx!(config.send(&cache_http, tag, &log).await);
                }
            }

            let mut log_buffer = String::new();
            {
                let mut xp_buffer = shared_xp_buffer.lock().unwrap();
                for (ign, diff, xp) in xp_buffer.values() {
                    let log = format!("**{}** contributed __{}__ xp, total *{}* xp", ign, diff, xp);
                    log_buffer.push('\n');
                    log_buffer.push_str(&log);
                }
                xp_buffer.clear();
            }
            if !log_buffer.is_empty() {
                let config = shared_config.read().await;
                let _ = ctx!(config.send(&cache_http, &TextChannelTag::XpLog, &log_buffer).await);
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
                let tag = some!(get_log_channel_tag(&event), continue);
                // Do not log if there are no log channels
                {
                    let config = config.read().await;
                    if config.text_channel_tags.tag_objects(&tag).next().is_none() {
                        continue;
                    }
                }

                match event {
                    WynnEvent::MemberContribute { id, ign, old_contrib, new_contrib } => {
                        let mut xp_buffer = xp_buffer.lock().unwrap();
                        let diff = new_contrib - old_contrib;
                        match xp_buffer.get_mut(id) {
                            Some(contrib) => {
                                if *ign != contrib.0 {
                                    contrib.0 = ign.clone();
                                }
                                contrib.1 += diff;
                                contrib.2 = *new_contrib;
                            }
                            None => {
                                xp_buffer.insert(id.clone(), (ign.clone(), diff, *new_contrib));
                            }
                        }
                    }
                    _ => {
                        let log = some!(make_wynn_log(&event), continue);
                        {
                            let mut buffers = buffers.lock().unwrap();
                            let buffer = buffers.get_mut(&tag).unwrap();
                            buffer.push('\n');
                            buffer.push_str(&log);
                        }
                    }
                }
            }
        }
    });
}

const SUMMARY_TABLE_LEN: usize = 30;

macro_rules! send_to_summary {
    ($cache_http:expr, $config:ident, $msg:expr) => {
        let config = $config.read().await;
        ok!(ctx!(config.send($cache_http, &TextChannelTag::Summary, $msg).await), continue);
    };
}

pub async fn start_summary_loop(
    cache_http: Arc<CacheAndHttp>, config: Arc<RwLock<Config>>, db: Arc<RwLock<DB>>,
) {
    tokio::spawn(async move {
        info!("Starting weekly summary loop");
        let mut receiver = {
            let db = db.read().await;
            db.signal.connect()
        };
        loop {
            let event =
                ok!(ctx!(receiver.recv().await, "Failed to receive db event in summary loop"), continue);

            if let DBEvent::WeeklyReset { message_lb, voice_lb, online_lb, xp_lb } = event.as_ref() {
                // Do not send summary if there are no channels to send
                {
                    let config = config.read().await;
                    if config.text_channel_tags.tag_objects(&TextChannelTag::Summary).next().is_none() {
                        continue;
                    }
                }

                let now = Utc::now().format("%Y %b %d");
                let msg = format!("> **Weekly summary for {}**\n\n__Weekly message__", now);
                send_to_summary!(&cache_http, config, &msg);

                ok!(send_summary(&cache_http, &config, &message_lb.0).await, continue);
                send_to_summary!(&cache_http, config, "__Weekly voice time__");
                ok!(send_summary(&cache_http, &config, &voice_lb.0).await, continue);
                send_to_summary!(&cache_http, config, "__Weekly online time__");
                ok!(send_summary(&cache_http, &config, &online_lb.0).await, continue);
                send_to_summary!(&cache_http, config, "__Weekly xp contribution__");
                ok!(send_summary(&cache_http, &config, &xp_lb.0).await, continue);
            }
        }
    });
}

async fn send_summary(
    cache_http: &CacheAndHttp, config: &RwLock<Config>, lb: &Vec<Vec<String>>,
) -> Result<()> {
    if lb.len() == 0 {
        let config = config.read().await;
        ctx!(
            config
                .send(&cache_http, &TextChannelTag::Summary, "```\nEmpty leaderboard\n```",)
                .await
        )?;
    }

    let max_widths = msgtool::table::calc_cols_max_width(lb);
    let tables = lb.chunks(SUMMARY_TABLE_LEN).map(|chunk| {
        let mut table = String::from("```\n");
        for row in chunk.iter().map(|row| msgtool::table::format_row(row, &max_widths)) {
            table.push_str(&row);
        }
        table.push_str("```");
        table
    });
    {
        let config = config.read().await;
        for table in tables {
            ctx!(config.send(&cache_http, &TextChannelTag::Summary, &table,).await)?;
        }
    }
    Ok(())
}
