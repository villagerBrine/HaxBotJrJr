use memberdb::discord::DiscordProfile;
use memberdb::guild::GuildProfile;
use memberdb::utils::Profiles;
use memberdb::wynn::WynnProfile;
use serenity::client::Context;

use util::some;

pub async fn get_names(ctx: &Context, profiles: &Profiles) -> (String, String) {
    let mut lower = "".to_string();

    match &profiles.member {
        Some(member) => lower.push_str(&member.rank.to_string()),
        None => {
            if let Some(guild) = &profiles.guild {
                lower.push_str(&guild.rank.to_string())
            }
        }
    }

    let discord_name = match &profiles.discord {
        Some(discord) => memberdb::utils::to_user(&ctx, discord.id)
            .map(|user| format!("{}#{}", user.name, user.discriminator)),
        None => None,
    };

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

pub fn get_guild_stat_fields<'a>(guild: &Option<GuildProfile>) -> Vec<(&'a str, String)> {
    match guild {
        Some(guild) => vec![
            ("Guild Rank", guild.rank.to_string()),
            ("Total XP Contributed", util::string::fmt_num(guild.xp, false)),
            ("Weekly XP Contributed", util::string::fmt_num(guild.xp_week, false)),
        ],
        None => Vec::new(),
    }
}

pub fn get_discord_stat_fields<'a>(discord: &Option<DiscordProfile>) -> Vec<(&'a str, String)> {
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

pub fn get_wynn_stat_fields<'a>(wynn: &Option<WynnProfile>) -> Vec<(&'a str, String)> {
    match wynn {
        Some(wynn) => vec![
            ("Total Online Time", util::string::fmt_second(wynn.activity)),
            ("Weekly Online Time", util::string::fmt_second(wynn.activity_week)),
        ],
        None => Vec::new(),
    }
}
