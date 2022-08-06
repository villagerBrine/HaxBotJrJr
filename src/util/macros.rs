#[macro_export]
/// Reply to message sender and exit command.
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
/// Reply to message sender
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

#[macro_export]
/// Get discord id and mc id via discord name and ign.
/// A discord member is also returned in case if you needs it.
/// Example
/// ```
/// let (member, discord_id, mcid): (Member, i64, String) = get_profile_ids!(
///     ctx, msg, guild, "MyDiscordName", "ign");
/// ```
macro_rules! get_profile_ids {
    ($ctx:ident, $msg:ident, $guild:ident, $client:ident, $discord_name:ident, $ign:ident) => {{
        let discord_member = util::some!(
            util::ctx!(util::discord::get_member_named(&$ctx.http, &$guild, &$discord_name).await)?,
            crate::finish!($ctx, $msg, "Failed to find an discord user with the given name")
        );
        let discord_id =
            util::ctx!(i64::try_from(discord_member.as_ref().user.id.0), "Failed to convert u64 into i64")?;

        let mcid = util::ok!(
            wynn::get_ign_id(&$client, &$ign).await,
            crate::finish!($ctx, $msg, "Provided mc ign doesn't exist")
        );

        (discord_member, discord_id, mcid)
    }};
}

#[macro_export]
/// Parse a target expression into `TargetId`
/// Example
/// ```
/// let db: RwLock<memberdb::DB> = ...;
/// let client: reqwest::Client = ...;
/// let target_id: TargetId = parse_user_target!(ctx, msg, db, client, guild, "d:test");
/// ```
macro_rules! parse_user_target {
    ($ctx:ident, $msg:ident, $db:ident, $client:ident, $guild:ident, $s:expr) => {{
        let target = match TargetObject::from_str(&$ctx, &$db, &$client, &$guild, $s).await {
            Ok(v) => v,
            Err(why) => finish!($ctx, $msg, format!("invalid target: {}", why)),
        };
        match target {
            TargetObject::Discord(DiscordObject::Member(member)) => {
                crate::util::db::TargetId::Discord(member.as_ref().user.id.clone())
            }
            TargetObject::Mc(id) => crate::util::db::TargetId::Wynn(id),
            _ => finish!($ctx, $msg, "Only discord/mc user are accepted as target"),
        }
    }};
}

#[macro_export]
/// Parse a target expression into member id
/// Example
/// ```
/// let db: RwLock<memberdb::DB> = ...;
/// let client: reqwest::Client = ...;
/// let member_id = parse_user_target!(ctx, msg, db, client, guild, "d:test");
/// ```
macro_rules! parse_user_target_mid {
    ($ctx:ident, $msg:ident, $db:ident, $client:ident, $guild:ident, $s:expr) => {{
        let target = crate::parse_user_target!($ctx, $msg, $db, $client, $guild, $s);
        util::some!(
            {
                let db = $db.read().await;
                target.get_mid(&db).await
            },
            finish!($ctx, $msg, "Failed to find target member in database")
        )
    }};
}
