/// Reply to message and exit command.
///
/// This the same as using [`send`] and then exit the command.
///
/// This macro takes a variable name that of [`Context`], and a variable name that of [`Message`],
/// which are used to send the reply.
/// It then takes the content of the reply, which can be a string, or a format string with
/// arguments.
/// ```
/// # use haxbotjr::finish;
/// use anyhow::Result;
/// use serenity::client::Context;
/// use serenity::model::channel::Message;
///
/// async fn do_stuff(ctx: &Context, msg: &Message, obj_ids: Vec<i64>) -> Result<()> {
///     let result: Result<()> = {
///         // do stuff
///         # Ok(())
///     };
///     match result {
///         Ok(_) => finish!(ctx, msg, "done"),
///         Err(why) => finish!(ctx, msg, "Error: {:#}", why),
///     }
/// }
/// ```
/// This macro takes care of error propagation, so it doesn't return anything, and must be used in
/// a function that returns [`Result`].
///
/// [`send`]: crate::send
/// [`Result`]: std::result::Result
/// [`Context`]: serenity::client::Context
/// [`Message`]: serenity::model::channel::Message
#[macro_export]
macro_rules! finish {
    ($ctx:ident, $sender:expr, $content:expr) => {{
        $crate::send!($ctx, $sender, $content);
        return Ok(());
    }};
    ($ctx:ident, $sender:expr, $($content:tt)+) => {{
        $crate::send!($ctx, $sender, $($content)+);
        return Ok(());
    }};
}

/// Reply to message.
///
/// This is the same as [`finish`] but it won't exit the command.
///
/// This macro takes a variable name that of [`Context`], and a variable name that of [`Message`],
/// which are used to send the reply.
/// It then takes the content of the reply, which can be a string, or a format string with
/// arguments.
/// ```
/// # use haxbotjr::send;
/// use anyhow::Result;
/// use serenity::client::Context;
/// use serenity::model::channel::Message;
///
/// async fn process_objects(ctx: &Context, msg: &Message, obj_ids: Vec<i64>) -> Result<()> {
///     send!(ctx, msg, "processing...");
///     // processing objects
///     send!(ctx, msg, "Processed {} objects", obj_ids.len());
///     Ok(())
/// }
/// ```
/// This macro takes care of error propagation, so it doesn't return anything, and must be used in
/// a function that returns [`Result`].
///
/// [`finish`]: crate::finish
/// [`Result`]: std::result::Result
/// [`Context`]: serenity::client::Context
/// [`Message`]: serenity::model::channel::Message
#[macro_export]
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

/// Same as [`anyhow::bail`] but works with [`CommandResult`]
///
/// [`CommandResult`]: serenity::framework::standard::CommandResult
#[macro_export]
macro_rules! cmd_bail {
    ($msg:literal $(,)?) => {
        return Err(anyhow::anyhow!($msg).into())
    };
    ($fmt:expr, $($arg:tt)+) => {
        return Err(anyhow::anyhow!($fmt, $($arg)+).into())
    };
}

/// Get bot data from [`Context`].
///
/// Takes a variable name that of [`Context`], and a list of data names.
/// Each data is represented by a data name, a string literal:
/// - "config": [`Arc<RwLock<Config>>`]
/// - "db": [`Arc<RwLock<DB>>`]
/// - "shard": [`Arc<Mutex<ShardManager>>`]
/// - "reqwest": [`reqwest::Client`]
/// - "vc": [`Arc<Mutex<VoiceTracker>>`]
/// - "cache": [`Arc<Cache>`]
/// ```
/// # use haxbotjr::data;
/// use anyhow::Result;
/// use serenity::client::Context;
///
/// async fn use_bot_data(ctx: &Context) -> Result<()> {
///     let db = data!(ctx, "db");
///     let (config, client) = data!(ctx, "config", "reqwest");
///     // Do stuffs with them
///     Ok(())
/// }
/// ```
/// This macro takes care of error propagation, so it doesn't return [`Result`], and must be used
/// in a function that returns [`Result`].
///
/// [`Result`]: std::result::Result
/// [`Context`]: serenity::client::Context
/// [`Arc<RwLock<Config>>`]: config::Config
/// [`Arc<RwLock<DB>>`]: memberdb::DB
/// [`Arc<Mutex<ShardManager>>`]: serenity::client::bridge::gateway::ShardManager
/// [`Arc<Mutex<VoiceTracker>>`]: memberdb::voice_tracker::VoiceTracker
/// [`Arc<Cache>`]: wynn::cache::Cache
#[macro_export]
macro_rules! data {
    ($ctx:ident, $name:tt) => {{
        let data = $ctx.data.read().await;
        $crate::data!(INTERNAL; $name, data)
    }};
    ($ctx:ident, $($name:tt),+) => {{
        let data = $ctx.data.read().await;
        ($($crate::data!(INTERNAL; $name, data),)+)
    }};
    (INTERNAL; "config", $data:ident) => {
        match $data.get::<config::Config>() {
            Some(v) => v.clone(),
            None => $crate::cmd_bail!("Failed to access config"),
        }
    };
    (INTERNAL; "db", $data:ident) => {
        match $data.get::<memberdb::DBContainer>() {
            Some(v) => v.clone(),
            None => $crate::cmd_bail!("Failed to access member db"),
        }
    };
    (INTERNAL; "shard", $data:ident) => {
        match $data.get::<$crate::data::ShardManagerContainer>() {
            Some(v) => v.clone(),
            None => $crate::cmd_bail!("Failed to access shard manager"),
        }
    };
    (INTERNAL; "reqwest", $data:ident) => {
        match $data.get::<$crate::data::ReqClientContainer>() {
            Some(v) => v.clone(),
            None => $crate::cmd_bail!("Failed to access reqwest client"),
        }
    };
    (INTERNAL; "vc", $data:ident) => {
        match $data.get::<memberdb::voice_tracker::VoiceTrackerContainer>() {
            Some(v) => v.clone(),
            None => $crate::cmd_bail!("Failed to access vc tracker"),
        }
    };
    (INTERNAL; "cache", $data:ident) => {
        match $data.get::<wynn::cache::Cache>() {
            Some(v) => v.clone(),
            None => $crate::cmd_bail!("Failed to access wynn cache"),
        }
    };
}

/// Send an embed.
///
/// Takes a variable name that of [`Context`], and a variable name that of [`Message`], both of
/// which are used to send the message.
/// It then takes a closure that takes one argument of type [`&mut CreateEmbed`], and returns a
/// [`&mut CreateEmbed`].
/// ```
/// # use haxbotjr::send_embed;
/// use anyhow::{Result, Context as AHContext};
/// use serenity::client::Context;
/// use serenity::model::channel::Message;
///
/// async fn send_example_embed(ctx: &Context, msg: &Message) -> Result<()> {
///     send_embed!(ctx, msg, |e| {
///         e.title("This is a title")
///             .field("a", "b", false)
///     });
///     Ok(())
/// }
/// ```
/// This macro takes care of error propagation, so it doesn't return anything, and must be used in
/// a function that returns [`Result`].
///
/// [`Result`]: std::result::Result
/// [`Context`]: serenity::client::Context
/// [`Message`]: serenity::model::channel::Message
/// [`&mut CreateEmbed`]: serenity::builder::CreateEmbed
#[macro_export]
macro_rules! send_embed {
    ($ctx:ident, $msg:ident, $embed_builder:expr) => {
        $msg.channel_id
            .send_message(&$ctx, |m| m.embed($embed_builder))
            .await
            .context("Failed to send embed")?
    };
}
