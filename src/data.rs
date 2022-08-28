//! Bot data initialization
use std::sync::Arc;
use std::time::Duration;

use memberdb::voice_tracker::{VoiceTracker, VoiceTrackerContainer};
use serenity::client::bridge::gateway::ShardManager;
use serenity::prelude::{Mutex as SMutex, TypeMapKey};
use serenity::Client;
use tokio::sync::{Mutex, RwLock};

use config::Config;
use event::timer::TimerSignal;
use event::{DiscordSignal, WynnSignal};
use memberdb::{DBContainer, DB};
use wynn::cache::Cache;

#[derive(Debug, Clone)]
/// Container for all bot data, so they can all be cloned at once.
pub struct BotData {
    pub db: Arc<RwLock<DB>>,
    pub config: Arc<RwLock<Config>>,
    pub reqwest_client: reqwest::Client,
    pub wynn_signal: WynnSignal,
    pub discord_signal: DiscordSignal,
    pub timer_signal: TimerSignal,
    pub wynn_cache: Arc<Cache>,
    pub voice_tracker: Arc<Mutex<VoiceTracker>>,
}

impl BotData {
    /// Initialize bot data
    pub async fn new(member_db_file: &str, config_file: &str) -> Self {
        let wynn_cache = Arc::new(Cache::new().await.expect("Failed to read wynn cache files"));
        let config = Config::new(config_file).expect("Failed to read config file");
        let config = Arc::new(RwLock::new(config));
        Self {
            db: DBContainer::new(member_db_file, 5).await,
            config,
            reqwest_client: ReqClientContainer::new().await,
            wynn_signal: WynnSignal::new(64),
            discord_signal: DiscordSignal::new(64),
            timer_signal: TimerSignal::new(1),
            wynn_cache,
            voice_tracker: VoiceTrackerContainer::new(),
        }
    }

    /// Added data to client
    pub async fn add_to_client(&self, client: &Client) {
        let mut data = client.data.write().await;
        data.insert::<DBContainer>(self.db.clone());
        data.insert::<Config>(self.config.clone());
        data.insert::<ReqClientContainer>(self.reqwest_client.clone());
        data.insert::<Cache>(self.wynn_cache.clone());
        data.insert::<ShardManagerContainer>(client.shard_manager.clone());
        data.insert::<VoiceTrackerContainer>(self.voice_tracker.clone());
    }
}

/// Bot data key for `ShardManager`
pub struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<SMutex<ShardManager>>;
}

/// Bot data key for `reqwest::Client`
pub struct ReqClientContainer;

impl TypeMapKey for ReqClientContainer {
    type Value = reqwest::Client;
}

impl ReqClientContainer {
    /// Create a new `reqwest::Client`
    pub async fn new() -> reqwest::Client {
        reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to build reqwest client")
    }
}
