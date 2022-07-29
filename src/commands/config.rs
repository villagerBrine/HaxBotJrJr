use anyhow::Context as AHContext;
use serenity::client::{Cache, Context};
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::{ChannelType, GuildChannel, Message};
use tokio::sync::RwLock;

use config::tag::{Tag, CHANNEL_TAGS, TEXT_CHANNEL_TAGS, USER_TAGS};
use config::utils::TagWrap;
use config::Config;
use msgtool::parser::DiscordObject;
use util::discord::PublicChannel;
use util::{ok, some};

use crate::checks::STAFF_CHECK;
use crate::{arg, cmd_bail, data, finish, send_embed};

#[command("tag")]
#[sub_commands(describe_tag, add_tag, remove_tag, show_tags, list_tagged)]
/// Display possible tags an object can have.
/// For operations on the tags, use subcommands.
async fn list_tags(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    send_embed!(ctx, msg, |e| {
        e.title("Tags")
            .description(
                "Use `tag info <tag>` to view description of a tag, \
for operations on those tags, like adding/removing tags, see `help tag` for more info.",
            )
            .field("Channel", util::string::str_list_iter(CHANNEL_TAGS.iter()), true)
            .field("Text Channel", util::string::str_list_iter(TEXT_CHANNEL_TAGS.iter()), true)
            .field("User", util::string::str_list_iter(USER_TAGS.iter()), true)
    });
    Ok(())
}

#[command("info")]
#[usage("<tag>")]
#[example("NoTrack")]
/// Describe what object the given tag can be attached to and what it means.
async fn describe_tag(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let content = match arg!(ctx, msg, args, TagWrap:"Invalid tag, use command `tag` to get list of tags") {
        TagWrap::User(t) => format!("User/role tag: {}", t.describe()),
        TagWrap::Channel(t) => format!("Channel/category tag: {}", t.describe()),
        TagWrap::TextChannel(t) => format!("Text channel tag: {}", t.describe()),
    };
    finish!(ctx, msg, content);
}

#[command("add")]
#[only_in(guild)]
#[checks(STAFF)]
#[usage("<tag> <target>")]
#[example("NoTrack #guild-chat")]
#[example("NoTrack c:THE GALAXY")]
#[example("NoNickUpdate d:Pucaet")]
#[example("NoRoleUpdate r:Cosmonaut")]
/// Add a tag to a target discord object.
///
/// > **How do I specify different targets**
/// - __Channel__: simply a link to it, ex: "#general"
/// - __Category__: "c:<category name>", ex: "c:Staff only"
/// (note that category name is displayed in all cap instead of its actual capitalization)
/// - __User__: "d:<username>", ex: "d:Pucaet"
/// - __Role__: "r:<role name>", ex: "r:Mission Specialist"
async fn add_tag(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let (tag, target_arg) = arg!(ctx, msg, args,
        TagWrap: "Invalid tag, use command `tag` to get list of tags",
        ..);
    let target = ok!(
        DiscordObject::from_str(&ctx, &guild, &target_arg).await,
        finish!(ctx, msg, "Invalid target, see `help tag add` for help")
    );
    let config = data!(ctx, "config");

    match target {
        DiscordObject::Member(member) => {
            let member = member.as_ref();
            match tag {
                TagWrap::User(tag) => {
                    let mut config = config.write().await;
                    config.user_tags.add(&member.user.id.0, tag);
                }
                _ => finish!(ctx, msg, "This tag can't be added to a discord user"),
            }
            finish!(ctx, msg, "Successfully added tag to user");
        }
        DiscordObject::Role(role) => {
            match tag {
                TagWrap::User(tag) => {
                    let mut config = config.write().await;
                    config.user_role_tags.add(&role.id.0, tag);
                }
                _ => finish!(ctx, msg, "This tag can't be added to a role"),
            }
            finish!(ctx, msg, "Successfully added tag to role");
        }
        DiscordObject::Channel(channel) => {
            match tag {
                TagWrap::Channel(tag) => {
                    let mut config = config.write().await;
                    config.channel_tags.add(&channel.id().0, tag);
                }
                TagWrap::TextChannel(tag) => {
                    let id = match channel {
                        PublicChannel::Guild(channel) => match channel.kind {
                            ChannelType::Text => &channel.id.0,
                            _ => finish!(ctx, msg, "This tag can only be added to text channels"),
                        },
                        _ => finish!(ctx, msg, "This tag can only be added to text channels"),
                    };
                    let mut config = config.write().await;
                    config.text_channel_tags.add(id, tag);
                }
                _ => finish!(ctx, msg, "This tag can't be added to a channel/category"),
            }
            finish!(ctx, msg, "Successfully added tag to channel/category");
        }
    }
}

#[command("remove")]
#[only_in(guild)]
#[checks(STAFF)]
#[usage("<tag> <target>")]
#[example("NoTrack #guild-chat")]
#[example("NoTrack c:THE GALAXY")]
#[example("NoNickUpdate d:Pucaet")]
#[example("NoRoleUpdate r:Cosmonaut")]
/// Remove a tag from a target discord object.
///
/// > **How do I specify different targets**
/// - __Channel__: simply a link to it, ex: "#general"
/// - __Category__: "c:<category name>", ex: "c:Staff only"
/// (note that category name is displayed in all cap instead of its actual capitalization)
/// - __User__: "d:<username>", ex: "d:Pucaet"
/// - __Role__: "r:<role name>", ex: "r:Mission Specialist"
async fn remove_tag(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let (tag, target_arg) = arg!(ctx, msg, args,
        TagWrap: "Invalid tag, use command `tag` to get list of tags",
        ..);
    let target = ok!(
        DiscordObject::from_str(&ctx, &guild, &target_arg).await,
        finish!(ctx, msg, "Invalid target, see `help tag remove` for help")
    );
    let config = data!(ctx, "config");

    match target {
        DiscordObject::Member(member) => {
            let member = member.as_ref();
            match tag {
                TagWrap::User(tag) => {
                    let mut config = config.write().await;
                    config.user_tags.remove(&member.user.id.0, &tag);
                }
                _ => finish!(ctx, msg, "An user can't possibly have this tag as it is incompatible"),
            }
            finish!(ctx, msg, "Successfully removed tag from user");
        }
        DiscordObject::Role(role) => {
            match tag {
                TagWrap::User(tag) => {
                    let mut config = config.write().await;
                    config.user_role_tags.remove(&role.id.0, &tag);
                }
                _ => finish!(ctx, msg, "A role can't possibly have this tag as it is incompatible"),
            }
            finish!(ctx, msg, "Successfully removed tag from role");
        }
        DiscordObject::Channel(channel) => {
            match tag {
                TagWrap::Channel(tag) => {
                    let mut config = config.write().await;
                    config.channel_tags.remove(&channel.id().0, &tag);
                }
                TagWrap::TextChannel(tag) => {
                    let mut config = config.write().await;
                    config.text_channel_tags.remove(&channel.id().0, &tag);
                }
                _ => {
                    finish!(ctx, msg, "A channel/category can't possibly have this tag as it is incompatible")
                }
            }
            finish!(ctx, msg, "Successfully removed tag from channel/category");
        }
    }
}

#[command("show")]
#[only_in(guild)]
#[usage("<target>")]
#[example("#guild-chat")]
#[example("c:THE GALAXY")]
#[example("d:Pucaet")]
#[example("r:Cosmonaut")]
/// Show a target object's attached tags.
///
/// > **How do I specify different targets**
/// - __Channel__: simply a link to it, ex: "#general"
/// - __Category__: "c:<category name>", ex: "c:Staff only"
/// (note that category name is displayed in all cap instead of its actual capitalization)
/// - __User__: "d:<username>", ex: "d:Pucaet"
/// - __Role__: "r:<role name>", ex: "r:Mission Specialist"
async fn show_tags(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let arg = args.rest();
    let guild = some!(msg.guild(&ctx), cmd_bail!("Failed to get message's guild"));
    let target = ok!(
        DiscordObject::from_str(&ctx, &guild, &arg).await,
        finish!(ctx, msg, "Invalid target, see `help tag show` for help")
    );
    let config = data!(ctx, "config");

    match target {
        DiscordObject::Member(member) => {
            let member = member.as_ref();

            let user_tags = {
                let config = config.read().await;
                config
                    .user_tags
                    .get(&member.user.id.0)
                    .map(|tags| util::string::str_list_iter(tags.iter()))
            };

            let role_tags: Vec<(String, String)> = {
                let config = config.read().await;
                member
                    .roles
                    .iter()
                    .filter_map(|id| {
                        config.user_role_tags.get(&id.0).map(|tags| {
                            let role_name = match id.to_role_cached(&ctx) {
                                Some(role) => role.name,
                                None => "UNKNOWN".to_string(),
                            };
                            (role_name, util::string::str_list_iter(tags.iter()))
                        })
                    })
                    .collect()
            };

            send_embed!(ctx, msg, |e| {
                if user_tags.is_none() && role_tags.is_empty() {
                    e.title("User has no tags")
                } else {
                    e.title("Tags on the user");
                    if let Some(tags) = user_tags {
                        e.field("From the user", tags, false);
                    }
                    for (role_name, tags) in role_tags {
                        e.field(format!("From user's role @{}", role_name), tags, false);
                    }
                    e
                }
            });
        }
        DiscordObject::Role(role) => {
            let list = {
                let config = config.read().await;
                config
                    .user_role_tags
                    .get(&role.id.0)
                    .map(|tags| util::string::str_list_iter(tags.iter()))
            };
            send_embed!(ctx, msg, |e| {
                if let Some(list) = list {
                    e.title("Tags on the role").description(list)
                } else {
                    e.title("Role has no tags")
                }
            });
        }
        DiscordObject::Channel(channel) => {
            let channel_tags = get_channel_tag_list(&config, &channel.id().0).await;
            match channel {
                PublicChannel::Category(_) => {
                    send_embed!(ctx, msg, |e| {
                        if let Some(tags) = channel_tags {
                            e.title("Tags on the category").description(tags)
                        } else {
                            e.title("Category has no tags")
                        }
                    });
                }
                PublicChannel::Guild(c) => {
                    let (category_tags, parent_tags) =
                        get_channel_parent_tag_lists(&ctx.cache, &config, c).await;
                    send_embed!(ctx, msg, |e| {
                        if category_tags.is_none() && parent_tags.is_none() && channel_tags.is_none() {
                            e.title("Channel has no tags")
                        } else {
                            e.title("Tags on the channel");
                            if let Some(tags) = channel_tags {
                                e.field("From the channel", tags, false);
                            }
                            if let Some(tags) = parent_tags {
                                e.field("From the parent channel", tags, false);
                            }
                            if let Some(tags) = category_tags {
                                e.field("From the category", tags, false);
                            }
                            e
                        }
                    });
                }
            }
        }
    }
    Ok(())
}

#[command("list")]
#[only_in(guild)]
#[usage("<tag>")]
#[example("NoNickUpdate")]
/// List all objects with this tag attached.
async fn list_tagged(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let tag = arg!(ctx, msg, args, TagWrap: "Invalid tag, use command `tag` to get list of tags");
    let config = data!(ctx, "config");

    let mut content = String::new();
    match tag {
        TagWrap::User(tag) => {
            {
                let config = config.read().await;
                for user_id in config.user_tags.tag_objects(&tag) {
                    content.push_str(&format!("<@!{}> ", user_id));
                }
            }
            {
                let config = config.read().await;
                for role_id in config.user_role_tags.tag_objects(&tag) {
                    content.push_str(&format!("<@&{}> ", role_id));
                }
            }
        }
        TagWrap::Channel(tag) => {
            let config = config.read().await;
            for channel_id in config.channel_tags.tag_objects(&tag) {
                content.push_str(&format!("<#{}> ", channel_id));
            }
        }
        TagWrap::TextChannel(tag) => {
            let config = config.read().await;
            for channel_id in config.text_channel_tags.tag_objects(&tag) {
                content.push_str(&format!("<#{}> ", channel_id));
            }
        }
    }
    finish!(ctx, msg, if content.is_empty() { "Empty" } else { &content });
}

async fn get_channel_parent_tag_lists(
    cache: &Cache, config: &RwLock<Config>, channel: &GuildChannel,
) -> (Option<String>, Option<String>) {
    let (category_id, parent_id) = util::discord::get_channel_parents(&cache, &channel);
    let category_tags = match category_id {
        Some(id) => {
            let config = config.read().await;
            config
                .channel_tags
                .get(&id.0)
                .map(|tags| util::string::str_list_iter(tags.iter()))
        }
        None => None,
    };
    let parent_tags = match parent_id {
        Some(id) => get_channel_tag_list(config, &id.0).await,
        None => None,
    };
    (category_tags, parent_tags)
}

async fn get_channel_tag_list(config: &RwLock<Config>, channel_id: &u64) -> Option<String> {
    let channel_tags = {
        let config = config.read().await;
        config
            .channel_tags
            .get(&channel_id)
            .map(|tags| util::string::str_list_iter(tags.iter()))
    };
    let text_channel_tags = {
        let config = config.read().await;
        config
            .text_channel_tags
            .get(&channel_id)
            .map(|tags| util::string::str_list_iter(tags.iter()))
    };

    if channel_tags.is_some() && text_channel_tags.is_some() {
        let mut channel_tags = channel_tags.unwrap();
        channel_tags.push_str(", ");
        channel_tags.push_str(&text_channel_tags.unwrap());
        return Some(channel_tags);
    }
    channel_tags.or(text_channel_tags)
}
