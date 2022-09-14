//! Commands for displaying member statistics
use std::fmt::Write as _;

use anyhow::Context as AHContext;
use serenity::client::Context;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::Message;

use memberdb::model::db::{Profiles, Stat};
use memberdb::model::discord::DiscordId;
use memberdb::query_builder::{Filter, QueryMod, Selectables, Sort};
use msgtool::pager::Pager;
use msgtool::table::{self, TableData};
use util::{ctx, some};

use crate::util::arg;
use crate::util::db::{self, TargetId};
use crate::util::discord::{MinimalLB, MinimalMembers};
use crate::{arg, cmd_bail, data, finish, flag, send_embed, t};

#[command("profile")]
#[bucket("mojang")]
#[only_in(guild)]
#[usage("[target]")]
#[example("m:Pucaet")]
#[example("d:Pucaet")]
#[example("d:Pucaet#9528")]
/// Display the profiles / statistics of `target`.
/// If `target` is not specified, then the discord user who called the command is used.
///
/// > **How do I specify different targets**
/// - __Discord user__: "d:<username>", ex: "d:Pucaet" or "d:Pucaet#9528"
/// - __Mc account__: "m:<ign>", ex: "m:SephDark18"
async fn display_profile(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get the associated guild"));
    let (db, client) = data!(ctx, "db", "reqwest");

    let target = {
        let arg = args.rest();
        if arg.is_empty() {
            TargetId::Discord(msg.author.id)
        } else {
            t!(db::parse_user_target(ctx, msg, &db, &client, &guild, args.rest()).await)
        }
    };
    let profiles = {
        let db = db.read().await;
        match target {
            TargetId::Discord(id) => {
                let id = DiscordId::try_from(id.0)?;
                Profiles::from_discord(&db, id).await
            }
            TargetId::Wynn(id) => Profiles::from_mc(&db, &id).await,
        }
    };

    if profiles.is_none() {
        finish!(ctx, msg, "No profiles found");
    }

    let names = msgtool::profile::get_names(&ctx.cache, &profiles).await;

    send_embed!(ctx, msg, |e| {
        e.author(|a| a.name(names.1)).title(names.0);

        if let Some(discord) = &profiles.discord {
            if let Some(user) = discord.id.to_user(&ctx.cache) {
                if let Some(url) = user.avatar_url() {
                    e.thumbnail(url);
                }
            }
        }

        for (name, value) in msgtool::profile::format_guild_stat_fields(&profiles.guild) {
            e.field(name, value, true);
        }
        for (name, value) in msgtool::profile::format_wynn_stat_fields(&profiles.wynn) {
            e.field(name, value, true);
        }
        for (name, value) in msgtool::profile::format_discord_stat_fields(&profiles.discord) {
            e.field(name, value, true);
        }

        if profiles.member.is_none() {
            e.footer(|f| f.text("This is an unlinked profile"));
        }

        e
    });

    Ok(())
}

#[command("members")]
#[usage("[filters] [minimal]")]
#[example("")]
#[example("minimal")]
#[example("Chief")]
#[example("guild >weekly_voice:1h")]
#[example("<Pilot xp")]
#[example(">Strategist <online:1w3d >xp:12m minimal")]
/// List members with optional filters.
///
/// If you use this command with "minimal" as an argument, then the table is displayed without any
/// styling. Useful if you are viewing it on a small screen.
///
/// > **"filters" can be any numbers of the following values separated by space**
/// `full`, `partial`, `guild`, `discord`, `wynn` (member type),
/// `Commander`, `Cosmonaut`, `Architect`, `Pilot`, `Rocketeer`, `Cadet` (member rank),
/// `Owner`, `Chief`, `Strategist`, `Captain`, `Recruiter`, `Recruit` (guild rank),
/// `in_guild` (is in guild), `has_mc`, `has_discord` (has linked profile)
///
/// Rank filters can also be written as `>Captain` to filter out all guild ranks below Captain,
/// or `<Cosmonaut` to filter out all member ranks above cosmonaut.
///
/// > **"filters" can also contains stat filters**
/// Following stats can be filtered: `message`, `weekly_message`, `voice`, `weekly_voice`, `online`,
/// `weekly_online`, `avg_online`, `xp`, `weekly_xp`.
///
/// With just the stat name, it filters out anyone with that stat as 0. Ex `online` filters out
/// anyone with no online time.
///
/// You can also filters for specific stat value by adding it after the name separated by `:`. Ex
/// `xp:1000` filters out anyone whose xp not equal to 1000.
///
/// Similar to rank filters, `>message:10` filters out anyone with message count below 10, and
/// `<weekly_voice:5m` filters out anyone with weekly voice time greater than 5 minutes. (Note that
/// the stat value has to be specified for it to work)
///
/// > **How to specify stat value**
/// For stats that is just a plain number (xp and message), you can just specify a number (`1000`).
/// You can also write `5,000,000` as `5m`, or `10,000,000,000` as `10b`.
/// Only whole integer is allows, and you can use commas to section up the number (`10,000,000`).
///
/// For stats that is a duration of time (voice and online), it can be specified in the format of
/// `(whole integer)(time unit)`, ex: `10h` is 10 hours.
/// Following time units are allows: `s` (second), `m` (minute), `h` (hour), `d` (day), and `w`
/// (week).
/// Multiple expressions can be chained together, ex: `1w5h20m` is 1 week 5 hours and 20 minutes.
async fn list_member(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let filters = arg::any::<Filter>(&mut args);
    let is_minimal = flag!(ctx, msg, args, "minimal");

    let db = data!(ctx, "db");

    let table = {
        let db = db.read().await;
        ctx!(memberdb::table::list_members(&ctx.cache, &db, &filters).await, "Failed to get members list")?
    };
    if table.is_empty() {
        finish!(ctx, msg, "Found 0 member");
    }

    let header = vec!["IGN".to_string(), "DISCORD".to_string(), "RANK".to_string()];
    crate::display_table_pages!(ctx, &msg.channel_id, table, header, 10, is_minimal, MinimalMembers);

    Ok(())
}

#[command("lb")]
#[usage("<stat> [filters] [minimal]")]
#[example("weekly_xp")]
#[example("xp minimal")]
#[example("message full")]
#[example("weekly_voice >Pilot <online:1w")]
#[example("online Recruiter >xp:10,000 voice:1d5h minimal")]
/// Display leaderboard on specified statistic with optional filters.
///
/// If you use this command with "minimal" as an argument, then the leaderboard is displayed without
/// any styling. Useful if you are viewing it on a small screen.
///
/// > **"stat" can be following values:**
/// `message`, `weekly_message`, `voice`, `weekly_voice`, `online`, `weekly_online`, `avg_online`,
/// `xp`, `weekly_xp`.
///
/// > **"filters" can be any numbers of the following values separated by space**
/// `full`, `partial`, `guild`, `discord`, `wynn` (member type),
/// `Commander`, `Cosmonaut`, `Architect`, `Pilot`, `Rocketeer`, `Cadet` (member rank),
/// `Owner`, `Chief`, `Strategist`, `Captain`, `Recruiter`, `Recruit` (guild rank),
/// `in_guild` (is in guild), `has_mc`, `has_discord` (has linked profile)
///
/// Rank filters can also be written as `>Captain` to filter out all guild ranks below Captain,
/// or `<Cosmonaut` to filter out all member ranks above cosmonaut.
///
/// > **"filters" can also contains stat filters**
/// With just the stat name, it filters out anyone with that stat as 0. Ex `online` filters out
/// anyone with no online time.
///
/// You can also filters for specific stat value by adding it after the name separated by `:`. Ex
/// `xp:1000` filters out anyone whose xp not equal to 1000.
///
/// Similar to rank filters, `>message:10` filters out anyone with message count below 10, and
/// `<weekly_voice:5m` filters out anyone with weekly voice time greater than 5 minutes. (Note that
/// the stat value has to be specified for it to work)
///
/// > **How to specify stat value**
/// For stats that is just a plain number (xp and message), you can just specify a number (`1000`).
/// You can also write `5,000,000` as `5m`, or `10,000,000,000` as `10b`.
/// Only whole integer is allows, and you can use commas to section up the number (`10,000,000`).
///
/// For stats that is a duration of time (voice and online), it can be specified in the format of
/// `(whole integer)(time unit)`, ex: `10h` is 10 hours.
/// Following time units are allows: `s` (second), `m` (minute), `h` (hour), `d` (day), and `w`
/// (week).
/// Multiple expressions can be chained together, ex: `1w5h20m` is 1 week 5 hours and 20 minutes.
async fn stat_leaderboard(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let stat = arg!(ctx, msg, args, "stat": Stat);
    let filters = arg::any::<Filter>(&mut args);
    let is_minimal = flag!(ctx, msg, args, "minimal");

    let db = data!(ctx, "db");

    let (table, header) = {
        let db = db.read().await;
        ctx!(
            memberdb::table::stat_leaderboard(&ctx.cache, &db, &stat, &filters).await,
            "Failed to get stat leaderboard"
        )?
    };
    if table.is_empty() {
        finish!(ctx, msg, "leaderboard empty");
    }

    crate::display_table_pages!(ctx, &msg.channel_id, table, header, 10, is_minimal, MinimalLB);

    Ok(())
}

#[command("member")]
#[bucket("mojang")]
#[only_in(guild)]
#[usage("<target>")]
#[example("m:Pucaet")]
#[example("d:Pucaet")]
#[example("d:Pucaet#9528")]
#[example("d:Cosmonaut Pucaet")]
/// Display info of member specified by `target`.
/// If what you want are statistics, use the command `profile` instead.
///
/// > **How do I specify different targets**
/// - __Discord user__: "d:<username>", ex: "d:Pucaet" or "d:Pucaet#9528"
/// - __Mc account__: "m:<ign>", ex: "m:SephDark18"
async fn display_member_info(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let (db, client) = data!(ctx, "db", "reqwest");

    let mid = t!(db::parse_user_target_mid(ctx, msg, &db, &client, &guild, args.rest()).await);
    let member = {
        let db = db.read().await;
        some!(
            ctx!(mid.get(&mut db.exe()).await, "Failed to get member from db")?,
            cmd_bail!("Failed to get member from db")
        )
    };

    let mut content =
        format!("**ID** `{}`\n**Rank** {}\n**Type** {}", member.id, member.rank, member.member_type);

    if let Some(mcid) = member.mcid {
        let db = db.read().await;
        let ign = ctx!(mcid.ign(&mut db.exe()).await, "Failed to get wynn.ign")?;
        write!(content, "\n**Minecraft** {} `{}`", ign, mcid)?;

        if let Ok(rank) = mcid.rank(&mut db.exe()).await {
            write!(content, "\n**Guild** {}", rank)?;
        }
    }
    if let Some(id) = member.discord {
        let user = some!(id.to_user(&ctx.cache), cmd_bail!("Failed to get discord user"));
        write!(content, "\n**Discord** {}#{} `{}`", user.name, user.discriminator, id)?;
    }

    finish!(ctx, msg, content)
}

#[command("table")]
#[usage("<columns> | [filters] | [sorts] [minimal]")]
#[example("weekly_xp")]
#[example("xp minimal")]
#[example("name message | full")]
#[example("weekly_voice | >Pilot <online:1w")]
#[example("ign guild_rank || ^online")]
#[example("name xp rank | >xp:10,000 voice:1d5h | rank ^xp minimal")]
/// Display a custom leaderboard.
///
/// If you use this command with "minimal" as an argument, then the leaderboard is displayed without
/// any styling. Useful if you are viewing it on a small screen.
///
/// This command has 3 separate argument lists separated by `|`, in order they are:
/// - __columns__ List of columns in the leaderboard
/// - __filters__ Filters to be applied
/// - __sorts__ How to sort the leaderboard
///
/// `|` can be omitted if you aren't skipping over any argument lists, ex: "table name xp", "table
/// name xp | partial".
/// If you have `sorts` and `filters` is empty, `|` still needs to be included, ex: "table name xp || ^xp".
///
/// > **"columns" can be any numbers of the following values separated by space**
/// `message`, `weekly_message`, `voice`, `weekly_voice`, `online`, `weekly_online`, `xp`,
/// `weekly_xp` (stats)
/// `mc_id`, `in_guild` (status on if member is in in-game guild), `ign`, `guild_rank`, `id`, `rank`,
/// `type`, `name` (member ign or discord username if ign not exist)
///
/// > **"filters" can be any numbers of the following values separated by space**
/// `full`, `partial`, `guild`, `discord`, `wynn` (member type),
/// `Commander`, `Cosmonaut`, `Architect`, `Pilot`, `Rocketeer`, `Cadet` (member rank),
/// `Owner`, `Chief`, `Strategist`, `Captain`, `Recruiter`, `Recruit` (guild rank),
/// `in_guild` (is in guild), `has_mc`, `has_discord` (has linked profile)
///
/// Rank filters can also be written as `>Captain` to filter out all guild ranks below Captain,
/// or `<Cosmonaut` to filter out all member ranks above cosmonaut.
///
/// > **"filters" can also contains stat filters**
/// With just the stat name, it filters out anyone with that stat as 0. Ex `online` filters out
/// anyone with no online time.
///
/// You can also filters for specific stat value by adding it after the name separated by `:`. Ex
/// `xp:1000` filters out anyone whose xp not equal to 1000.
///
/// Similar to rank filters, `>message:10` filters out anyone with message count below 10, and
/// `<weekly_voice:5m` filters out anyone with weekly voice time greater than 5 minutes. (Note that
/// the stat value has to be specified for it to work)
///
/// > **How to specify stat value**
/// For stats that is just a plain number (xp and message), you can just specify a number (`1000`).
/// You can also write `5,000,000` as `5m`, or `10,000,000,000` as `10b`.
/// Only whole integer is allows, and you can use commas to section up the number (`10,000,000`).
///
/// For stats that is a duration of time (voice and online), it can be specified in the format of
/// `(whole integer)(time unit)`, ex: `10h` is 10 hours.
/// Following time units are allows: `s` (second), `m` (minute), `h` (hour), `d` (day), and `w`
/// (week).
/// Multiple expressions can be chained together, ex: `1w5h20m` is 1 week 5 hours and 20 minutes.
///
/// > **"sorts" can be any number of column names separated by space**
/// With just the column name, that column is ordered in descent order. If `^` is added to the
/// front (`^xp`), then that column is ordered in ascend order.
/// Sorts are applied in the order they are specified in.
/// Note that the column `name` is special and can't be sorted.
async fn display_table(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let columns = arg::any::<Selectables>(&mut args);
    arg::consume_raw(&mut args, "|");
    let filters = arg::any::<Filter>(&mut args);
    arg::consume_raw(&mut args, "|");
    let sorts = arg::any::<Sort>(&mut args);
    let is_minimal = flag!(ctx, msg, args, "minimal");

    if columns.is_empty() {
        finish!(ctx, msg, "No columns specified");
    }
    let mut actions = Vec::with_capacity(filters.len() + sorts.len());
    actions.append(&mut filters.into_iter().map(QueryMod::Filter).collect());
    actions.append(&mut sorts.into_iter().map(QueryMod::Sort).collect());

    let db = data!(ctx, "db");

    let (table, header) = {
        let db = db.read().await;
        ctx!(
            memberdb::table::make_table(&ctx.cache, &db, &columns, &actions).await,
            "Failed to get stat leaderboard"
        )?
    };
    if table.is_empty() {
        finish!(ctx, msg, "leaderboard empty");
    }

    crate::display_table_pages!(ctx, &msg.channel_id, table, header, 10, is_minimal, MinimalLB);

    Ok(())
}

#[macro_export]
/// Display a table as paged message.
macro_rules! display_table_pages {
    ($ctx:ident, $channel_id:expr, $data:ident, $header:ident, $page_len:literal, $is_minimal:ident, $minimal_wrap:ident) => {{
        let data = table::borrow_table(&$data);
        let header = table::borrow_row(&$header);
        let table_data = TableData::paginate(data, header, $page_len);
        if $is_minimal {
            let table_data = table_data
                .into_iter()
                .map(|data| $minimal_wrap(data.0))
                .collect::<Vec<$minimal_wrap>>();
            let mut pager = Pager::new(table_data);
            ctx!(
                msgtool::interact::page(&$ctx, $channel_id, &mut pager, 120).await,
                "Error when displaying leaderboard pages"
            )?;
        } else {
            let mut pager = Pager::new(table_data);
            ctx!(
                msgtool::interact::page(&$ctx, $channel_id, &mut pager, 120).await,
                "Error when displaying leaderboard pages"
            )?;
        };
    }};
}
