#[macro_export]
/// Reply to message and exit command.
///
/// Examples
/// ```
/// finish!(ctx, msg, "done");
/// finish!(ctx, msg, "Error: {:#}", why);
/// ```
macro_rules! finish {
    ($ctx:ident, $sender:expr, $content:expr) => {{
        crate::send!($ctx, $sender, $content);
        return Ok(());
    }};
    ($ctx:ident, $sender:expr, $($content:tt)+) => {{
        crate::send!($ctx, $sender, $($content)+);
        return Ok(());
    }};
}

#[macro_export]
/// Reply to message.
/// This is equivalent to `ctx!(msg.reply(ctx, format!(...)), ...)?`
///
/// Examples
/// ```
/// send!(ctx, msg, "processing...");
/// send!(ctx, msg, "Reading {}/{} object", index, size);
/// ```
macro_rules! send {
    ($ctx:ident, $sender:expr, $content:expr) => {
        $sender.reply(&$ctx, $content).await.map_err(|why| {
            tracing::error!("Failed to reply to message: {:#}", why);
            why
        })?
    };
    ($ctx:ident, $sender:expr, $($content:tt)+) => {
        $sender.reply(&$ctx, format!($($content)+)).await.map_err(|why| {
            tracing::error!("Failed to reply to message: {:#}", why);
            why
        })?
    };
}

#[macro_export]
/// like `anyhow::bail!` but works with `CommandResult`
macro_rules! cmd_bail {
    ($msg:literal $(,)?) => {
        return Err(anyhow::anyhow!($msg).into())
    };
    ($fmt:expr, $($arg:tt)+) => {
        return Err(anyhow::anyhow!($fmt, $($arg)+).into())
    };
}

#[macro_export]
/// Get bot data from `Context`.
///
/// Each data is represented by a string literal:
/// - "config": `Arc<RwLock<config::Config>>`
/// - "db": `Arc<RwLock<memberdb::DB>>`
/// - "shard": `Arc<Mutex<ShardManager>>`
/// - "reqwest": `reqwest::Client`
/// - "vc": `Arc<Mutex<VoiceTracker>>`
/// - "cache": `Arc<Cache>`
///
/// Examples
/// ```
/// let db = data!(ctx, "db");
/// let (config, client) = data!(ctx, "config", "reqwest");
/// ```
macro_rules! data {
    ($ctx:ident, $name:tt) => {{
        let data = $ctx.data.read().await;
        crate::data!(INTERNAL; $name, data)
    }};
    ($ctx:ident, $($name:tt),+) => {{
        let data = $ctx.data.read().await;
        ($(crate::data!(INTERNAL; $name, data),)+)
    }};
    (INTERNAL; "config", $data:ident) => {
        match $data.get::<config::ConfigContainer>() {
            Some(v) => v.clone(),
            None => crate::cmd_bail!("Failed to access config"),
        }
    };
    (INTERNAL; "db", $data:ident) => {
        match $data.get::<memberdb::DBContainer>() {
            Some(v) => v.clone(),
            None => crate::cmd_bail!("Failed to access member db"),
        }
    };
    (INTERNAL; "shard", $data:ident) => {
        match $data.get::<crate::data::ShardManagerContainer>() {
            Some(v) => v.clone(),
            None => crate::cmd_bail!("Failed to access shard manager"),
        }
    };
    (INTERNAL; "reqwest", $data:ident) => {
        match $data.get::<crate::data::ReqClientContainer>() {
            Some(v) => v.clone(),
            None => crate::cmd_bail!("Failed to access reqwest client"),
        }
    };
    (INTERNAL; "vc", $data:ident) => {
        match $data.get::<memberdb::voice_tracker::VoiceTrackerContainer>() {
            Some(v) => v.clone(),
            None => crate::cmd_bail!("Failed to access vc tracker"),
        }
    };
    (INTERNAL; "cache", $data:ident) => {
        match $data.get::<wynn::cache::WynnCacheContainer>() {
            Some(v) => v.clone(),
            None => crate::cmd_bail!("Failed to access wynn cache"),
        }
    };
}

#[macro_export]
/// Send an embed.
///
/// Example
/// ```
/// send_embed!(ctx, msg, |e: CreateEmbed| {
///     e.title("This is a title")
///         .field("a", "b", false)
/// });
/// ```
macro_rules! send_embed {
    ($ctx:ident, $msg:ident, $embed_builder:expr) => {
        $msg.channel_id
            .send_message(&$ctx, |m| m.embed($embed_builder))
            .await
            .context("Failed to send embed")?
    };
}
