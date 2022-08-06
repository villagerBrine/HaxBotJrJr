use std::env;

use serenity::framework::standard::macros::group;
use serenity::http::Http;
use tokio::time::{self, Duration};
use tracing::{error, info};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, Layer};

use haxbotjr::commands::*;
use haxbotjr::data::BotData;

#[group]
#[commands(ping, set_custom_nick, display_online_players)]
struct General;

#[group]
#[commands(display_profile, stat_leaderboard, display_table)]
struct Statistics;

#[group]
#[commands(list_member, display_member_info)]
struct Members;

#[group("Member Management")]
#[commands(
    link_profile,
    unlink_profile,
    add_partial,
    add_member,
    remove_member,
    set_member_rank,
    promote_member,
    demote_member,
    fix_nick,
    fix_role,
    sync_member_ign
)]
struct MemberManagement;

#[group]
#[commands(get_rank_symbols, utc_now)]
struct Utilities;

#[group]
#[commands(list_tags)]
struct Configuration;

#[group]
#[owners_only]
#[commands(sql, check_db_integrity)]
struct Owner;

#[tokio::main]
async fn main() {
    // Loaded ".env"
    dotenv::dotenv().expect("Failed to load .env file");

    // Initialize logging
    let file_appender = tracing_appender::rolling::daily("./log", "log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(
                fmt::Layer::default()
                    .with_ansi(false)
                    .with_timer(fmt::time::UtcTime::rfc_3339())
                    .with_writer(file_writer)
                    .with_filter(LevelFilter::INFO),
            )
            .with(
                fmt::Layer::default()
                    .with_ansi(true)
                    .with_timer(fmt::time::UtcTime::rfc_3339())
                    .with_writer(std::io::stdout)
                    .with_filter(LevelFilter::INFO),
            ),
    )
    .expect("Failed to set global log subscriber");

    // Get global variables
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let http = Http::new(&token);

    // Creating client
    let bot_data = BotData::new("./database/member.db", "./config.json").await;
    let framework = haxbotjr::my_framework(&http)
        .await
        .help(&MY_HELP)
        .group(&GENERAL_GROUP)
        .group(&STATISTICS_GROUP)
        .group(&MEMBERS_GROUP)
        .group(&MEMBERMANAGEMENT_GROUP)
        .group(&CONFIGURATION_GROUP)
        .group(&UTILITIES_GROUP)
        .group(&OWNER_GROUP)
        // Rate limit for mojang api, 1 request lower just in case
        .bucket("mojang", |b| b.time_span(600).limit(599))
        .await;
    let mut client = haxbotjr::my_client(&token, framework, bot_data.discord_signal.clone())
        .await
        .expect("Failed to create client");
    bot_data.add_to_client(&client).await;

    // Start loops
    let data = bot_data.clone();
    event::timer::start_loop(data.timer_signal).await;

    let data = bot_data.clone();
    let cache_http = client.cache_and_http.clone();
    haxbotjr::logging::start_log_loop(cache_http, data.config, data.wynn_signal).await;

    let data = bot_data.clone();
    let cache_http = client.cache_and_http.clone();
    haxbotjr::logging::start_summary_loop(cache_http, data.config, data.db).await;

    let data = bot_data.clone();
    let cache_http = client.cache_and_http.clone();
    haxbotjr::loops::start_loops(cache_http, data.db, data.config, data.wynn_signal, data.discord_signal)
        .await;

    let data = bot_data.clone();
    let cache = client.cache_and_http.cache.clone();
    memberdb::loops::start_loops(
        data.db,
        data.config,
        cache,
        data.voice_tracker,
        data.wynn_signal,
        data.discord_signal,
        data.timer_signal,
    )
    .await;

    let data = bot_data.clone();
    config::start_loop(data.config, data.discord_signal).await;

    let data = bot_data.clone();
    wynn::loops::start_loops(data.wynn_signal, data.reqwest_client, data.wynn_cache, data.db).await;

    let data = bot_data.clone();
    tokio::spawn(async move {
        info!("Starting periodic state saving loop");
        let mut interval = time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            data.wynn_cache.store().await;
            data.config.read().await.store("./config.json");
        }
    });

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        info!("Starting ctrl+c handler");
        tokio::signal::ctrl_c().await.expect("Could not register ctrl+c handler");

        // shutdown codes
        info!("Saving api cache files");
        bot_data.wynn_cache.store().await;
        info!("Saving config file");
        bot_data.config.read().await.store("./config.json");
        shard_manager.lock().await.shutdown_all().await;
    });

    // Start client
    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
