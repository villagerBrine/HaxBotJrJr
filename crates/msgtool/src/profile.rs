//! Utilities for formatting string presentation of database profiles
use memberdb::model::db::Profiles;
use memberdb::model::discord::DiscordProfile;
use memberdb::model::guild::GuildProfile;
use memberdb::model::wynn::WynnProfile;
use serenity::client::Cache;

use util::some;

/// Get the name of a member given its profiles.
///
/// Returns 2 strings.
///
/// The first string is the name, and is formatted as:
/// "(member or guild rank) (ign or discord username)"
/// With discord username formatted as "username#discriminator".
///
/// Member rank is prioritized over guild rank, and if both doesn't exists, then the rank portion
/// of the name is empty. Ign is prioritized over discord username.
///
/// If the member have both an ign and a discord username, then their discord username is returned
/// as the second string in case you needs it.
pub async fn get_names(cache: &Cache, profiles: &Profiles) -> (String, String) {
    let mut name = "".to_string();

    // Push member rank, if not exist, try push guild rank
    match &profiles.member {
        Some(member) => name.push_str(&member.rank.to_string()),
        None => {
            if let Some(guild) = &profiles.guild {
                name.push_str(&guild.rank.to_string())
            }
        }
    }

    // Get and format discord username
    let discord_name = match &profiles.discord {
        Some(discord) => discord
            .id
            .to_user(cache)
            .map(|user| format!("{}#{}", user.name, user.discriminator)),
        None => None,
    };

    // Push ign, if not exist, try push discord username.
    // The upper name is already returned in the process
    let remain = match &profiles.wynn {
        Some(wynn) => {
            name.push(' ');
            name.push_str(&wynn.ign);
            some!(discord_name, "".to_string())
        }
        None => {
            if let Some(discord_name) = discord_name {
                name.push(' ');
                name.push_str(&discord_name);
            }
            "".to_string()
        }
    };

    (name, remain)
}

/// Format guild stats.
///
/// Given a guild profile, return all its stats as list of tuples in the form of:
/// (stat name, formatted stat).
/// ```
/// use memberdb::model::guild::{GuildProfile, GuildRank};
/// use msgtool::profile::format_guild_stat_fields;
///
/// let profile = GuildProfile {
///     id: "3f6dc89b-444d-4f28-b1dd-c3cac33ea152".to_string(),
///     mid: Some(24),
///     rank: GuildRank::Chief,
///     xp: 1234567,
///     xp_week: 123,
/// };
///
/// assert!(format_guild_stat_fields(&Some(profile)) == vec! [
///     ("Guild Rank", "Chief".to_string()),
///     ("Total XP Contributed", "1,234,567".to_string()),
///     ("Weekly XP Contributed", "123".to_string()),
/// ]);
/// assert!(format_guild_stat_fields(&None).is_empty());
/// ```
pub fn format_guild_stat_fields(guild: &Option<GuildProfile>) -> Vec<(&str, String)> {
    match guild {
        Some(guild) => vec![
            ("Guild Rank", guild.rank.to_string()),
            ("Total XP Contributed", util::string::fmt_num(guild.xp, false)),
            ("Weekly XP Contributed", util::string::fmt_num(guild.xp_week, false)),
        ],
        None => Vec::new(),
    }
}

/// Format discord stats.
///
/// Given a discord profile, return all its stats as list of tuples in the form of:
/// (stat name, formatted stat).
///
/// Note that only the used stats are formatted.
/// ```
/// use memberdb::model::discord::DiscordProfile;
/// use msgtool::profile::format_discord_stat_fields;
///
/// let profile = DiscordProfile {
///     id: 658478931682394134,
///     mid: Some(24),
///     message: 1234567,
///     message_week: 123,
///     image: 0,
///     reaction: 0,
///     voice: 70,
///     voice_week: 12,
///     activity: 0,
/// };
///
/// assert!(format_discord_stat_fields(&Some(profile)) == vec! [
///     ("Total Messages", "1,234,567".to_string()),
///     ("Weekly Messages", "123".to_string()),
///     ("Total Voice Time", "1m 10s".to_string()),
///     ("Weekly Voice Time", "12s".to_string()),
/// ]);
/// assert!(format_discord_stat_fields(&None).is_empty());
/// ```
pub fn format_discord_stat_fields(discord: &Option<DiscordProfile>) -> Vec<(&str, String)> {
    match discord {
        Some(discord) => vec![
            ("Total Messages", util::string::fmt_num(discord.message, false)),
            ("Weekly Messages", util::string::fmt_num(discord.message_week, false)),
            ("Total Voice Time", util::string::fmt_second(discord.voice)),
            ("Weekly Voice Time", util::string::fmt_second(discord.voice_week)),
        ],
        None => Vec::new(),
    }
}

/// Format wynn stats.
///
/// Given a wynn profile, return all its stats as list of tuples in the form of:
/// (stat name, formatted stat).
/// ```
/// use memberdb::model::wynn::WynnProfile;
/// use msgtool::profile::format_wynn_stat_fields;
///
/// let profile = WynnProfile {
///     id: "3f6dc89b-444d-4f28-b1dd-c3cac33ea152".to_string(),
///     mid: Some(24),
///     guild: true,
///     ign: "Pucaet".to_string(),
///     emerald: 0,
///     emerald_week: 0,
///     activity: 70,
///     activity_week: 12,
/// };
///
/// assert!(format_wynn_stat_fields(&Some(profile)) == vec! [
///     ("Total Online Time", "1m 10s".to_string()),
///     ("Weekly Online Time", "12s".to_string()),
/// ]);
/// assert!(format_wynn_stat_fields(&None).is_empty());
/// ```
pub fn format_wynn_stat_fields(wynn: &Option<WynnProfile>) -> Vec<(&str, String)> {
    match wynn {
        Some(wynn) => vec![
            ("Total Online Time", util::string::fmt_second(wynn.activity)),
            ("Weekly Online Time", util::string::fmt_second(wynn.activity_week)),
        ],
        None => Vec::new(),
    }
}
