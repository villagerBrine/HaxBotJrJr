//! Tools for live interactions with users via discord messages, mostly through message components
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serenity::builder::{CreateActionRow, CreateButton, CreateComponents};
use serenity::client::bridge::gateway::ShardMessenger;
use serenity::futures::StreamExt;
use serenity::http::{CacheHttp, Http};
use serenity::model::application::component::ButtonStyle;
use serenity::model::application::interaction::message_component::MessageComponentInteraction;
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::id::{ChannelId, UserId};

use crate::pager::{Pager, ToPage};

/// Color styles for confirm buttons of message components
#[derive(Debug)]
pub enum ConfirmStyle {
    /// Normal green/red button style
    Normal,
    /// Button style with a focus on the confirm button and it is colored red
    Important,
}

impl ConfirmStyle {
    /// Get the color of the "yes" button
    pub fn yes(&self) -> ButtonStyle {
        match self {
            Self::Normal => ButtonStyle::Success,
            Self::Important => ButtonStyle::Danger,
        }
    }

    /// Get the color of the "no" button
    pub fn no(&self) -> ButtonStyle {
        match self {
            Self::Normal => ButtonStyle::Danger,
            Self::Important => ButtonStyle::Secondary,
        }
    }
}

/// Ask user for confirmation via message.
///
/// Send a message that asks the user for yes or no, and return the answer in boolean.
/// The message is stop being observed after `timeout` (in seconds) is elapsed.
pub async fn confirm<C>(
    ctx: &C, channel_id: &ChannelId, content: &str, style: &ConfirmStyle, timeout: u64, user_id: UserId,
) -> Result<Option<(bool, Arc<MessageComponentInteraction>)>>
where
    C: AsRef<Http> + AsRef<ShardMessenger> + CacheHttp,
{
    let m = channel_id
        .send_message(ctx, |m| {
            m.content(content).components(|c| {
                c.create_action_row(|ar| {
                    let mut yes = CreateButton::default();
                    yes.custom_id("YES").emoji('✅').label("YES").style(style.yes());
                    ar.add_button(yes);

                    let mut no = CreateButton::default();
                    no.custom_id("NO").emoji('❎').label("NO").style(style.no());
                    ar.add_button(no);

                    ar
                })
            })
        })
        .await?;

    let ci = match m
        .await_component_interaction(ctx)
        .timeout(Duration::from_secs(timeout))
        .author_id(user_id)
        .await
    {
        Some(ci) => {
            ci.create_interaction_response(ctx, |r| {
                r.kind(InteractionResponseType::UpdateMessage)
                    .interaction_response_data(|d| d.set_components(CreateComponents::default()))
            })
            .await?;
            ci
        }
        None => {
            m.reply(ctx, "Timed out").await?;
            return Ok(None);
        }
    };

    let choice = ci.data.custom_id == "YES";
    Ok(Some((choice, ci)))
}

/// Send a paged message.
///
/// Send a navigable paged message using [`Pager`].
/// The message is stop being observed after `timeout` (in seconds) is elapsed.
///
/// [`Pager`]: crate::pager::Pager
pub async fn page<C, D>(
    ctx: &C, channel_id: &ChannelId, pager: &mut Pager<D, String>, timeout: u64,
) -> Result<()>
where
    C: AsRef<Http> + AsRef<ShardMessenger> + CacheHttp,
    D: ToPage<Page = String>,
{
    let content = pager.get_page();
    if pager.len() == 1 {
        channel_id.say(ctx, content).await?;
        return Ok(());
    }
    let msg = channel_id
        .send_message(ctx, |m| {
            m.content(content)
                .components(|c| c.create_action_row(|ar| create_page_buttons(ar, 0, 2)))
        })
        .await?;

    let mut cib = msg
        .await_component_interactions(ctx)
        .timeout(Duration::from_secs(timeout))
        .build();
    while let Some(mci) = cib.next().await {
        match mci.data.custom_id.as_str() {
            "FIRST" => {
                pager.first();
                update_page_message(mci, ctx, pager).await?;
            }
            "PREV" => {
                pager.prev();
                update_page_message(mci, ctx, pager).await?;
            }
            "NEXT" => {
                pager.next();
                update_page_message(mci, ctx, pager).await?;
            }
            "LAST" => {
                pager.last();
                update_page_message(mci, ctx, pager).await?;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Updates paged message
async fn update_page_message<D>(
    mci: Arc<MessageComponentInteraction>, http: &impl AsRef<Http>, pager: &Pager<D, String>,
) -> Result<()>
where
    D: ToPage<Page = String>,
{
    Ok(mci
        .create_interaction_response(http, |r| {
            r.kind(InteractionResponseType::UpdateMessage).interaction_response_data(|d| {
                d.content(pager.get_page()).components(|c| {
                    let mut ar = CreateActionRow::default();
                    create_page_buttons(&mut ar, pager.index(), pager.len());
                    c.set_action_row(ar)
                })
            })
        })
        .await?)
}

/// Create paged message buttons
fn create_page_buttons(ar: &mut CreateActionRow, index: usize, len: usize) -> &mut CreateActionRow {
    let mut first = CreateButton::default();
    first
        .custom_id("FIRST")
        .style(ButtonStyle::Secondary)
        .label("First")
        .disabled(index == 0);
    ar.add_button(first);

    let mut prev = CreateButton::default();
    prev.custom_id("PREV").style(ButtonStyle::Secondary).label("Previous");
    ar.add_button(prev);

    let mut next = CreateButton::default();
    next.custom_id("NEXT").style(ButtonStyle::Secondary).label("Next");
    ar.add_button(next);

    let mut last = CreateButton::default();
    last.custom_id("LAST")
        .style(ButtonStyle::Secondary)
        .label("Last")
        .disabled(index == len - 1);
    ar.add_button(last);

    ar
}
