//! Utilities for string presentation of database profiles
use memberdb::discord::DiscordProfile;
use memberdb::guild::GuildProfile;
use memberdb::utils::Profiles;
use memberdb::wynn::WynnProfile;
use serenity::client::Cache;

use util::some;

/// Get the name of a member given its profiles.
/// Name is composed of two parts, the upper part, and the lower part.
///
/// Lower part is formatted as `(member or guild rank) (ign or discord username)`.
/// Member rank is prioritized over guild rank, and if both doesn't exists, then the rank portion
/// of the string is cut.
/// Ign is prioritized over discord username
///
/// If the member have both an ign and a discord username, then the upper part is set to the one
/// that isn't included in the lower part, otherwise it is empty.
pub async fn get_names(cache: &Cache, profiles: &Profiles) -> (String, String) {
    let mut lower = "".to_string();

    // Push member rank, if not exist, try push guild rank
    match &profiles.member {
        Some(member) => lower.push_str(&member.rank.to_string()),
        None => {
            if let Some(guild) = &profiles.guild {
                lower.push_str(&guild.rank.to_string())
            }
        }
    }

    // Get and format discord username
    let discord_name = match &profiles.discord {
        Some(discord) => memberdb::utils::to_user(cache, discord.id)
            .map(|user| format!("{}#{}", user.name, user.discriminator)),
        None => None,
    };

    // Push ign, if not exist, try push discord username.
    // The upper name is already returned in the process
    let upper = match &profiles.wynn {
        Some(wynn) => {
            lower.push(' ');
            lower.push_str(&wynn.ign);
            some!(discord_name, "".to_string())
        }
        None => {
            if let Some(name) = discord_name {
                lower.push(' ');
                lower.push_str(&name);
            }
            "".to_string()
        }
    };

    (upper, lower)
}

/// Given a guild profiles, and return all its stats as list of tuples: (stat name, formatted stat)
pub fn get_guild_stat_fields(guild: &Option<GuildProfile>) -> Vec<(&str, String)> {
    match guild {
        Some(guild) => vec![
            ("Guild Rank", guild.rank.to_string()),
            ("Total XP Contributed", util::string::fmt_num(guild.xp, false)),
            ("Weekly XP Contributed", util::string::fmt_num(guild.xp_week, false)),
        ],
        None => Vec::new(),
    }
}

/// Given a discord profiles, and return all its stats as list of tuples: (stat name, formatted stat)
pub fn get_discord_stat_fields(discord: &Option<DiscordProfile>) -> Vec<(&str, String)> {
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

/// Given a wynn profiles, and return all its stats as list of tuples: (stat name, formatted stat)
pub fn get_wynn_stat_fields(wynn: &Option<WynnProfile>) -> Vec<(&str, String)> {
    match wynn {
        Some(wynn) => vec![
            ("Total Online Time", util::string::fmt_second(wynn.activity)),
            ("Weekly Online Time", util::string::fmt_second(wynn.activity_week)),
        ],
        None => Vec::new(),
    }
}
