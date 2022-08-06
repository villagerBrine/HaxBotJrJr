//! Dev util commands
use std::process::Command;

use anyhow::Context as AHContext;
use serenity::client::Context;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::Message;

use util::ctx;

use crate::{arg, data, finish, send};

#[command]
/// Run sql query and send its output as message.
async fn sql(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let (db_name, mut query) = arg!(ctx, msg, args, "database name", "query");

    query.push(';');
    let output = Command::new("sqlite3")
        .args([format!("database/{}.db", db_name), query])
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;
    if !stdout.is_empty() {
        finish!(ctx, msg, stdout);
    }

    let stderr = String::from_utf8(output.stderr)?;
    if !stderr.is_empty() {
        finish!(ctx, msg, stderr);
    }

    finish!(ctx, msg, "No output");
}

#[command("dbCheck")]
/// Check for database integrity.
async fn check_db_integrity(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    let db = data!(ctx, "db");

    let mut msg = send!(ctx, msg, "Verifying");

    {
        let db = db.read().await;
        // Check if member type mismatches linked profiles
        let rows = sqlx::query!(
            "SELECT oid FROM member WHERE \
                                (discord NOT NULL AND mcid NOT NULL AND type!='full') OR \
                                (discord NOT NULL AND mcid IS NULL AND type!='discord') OR \
                                (discord IS NULL AND mcid NOT NULL AND \
                                    NOT (SELECT guild FROM wynn WHERE id=member.mcid) AND type!='wynn') OR \
                                (discord IS NULL AND mcid NOT NULL AND \
                                    (SELECT guild FROM wynn WHERE id=member.mcid) AND type!='guild')"
        )
        .fetch_all(&db.pool)
        .await
        .context("")?;

        if !rows.is_empty() {
            send!(ctx, msg, format!("member wrong member type: {:?}", rows));
        }

        // Checks if member has no linked profiles
        let rows = sqlx::query!("SELECT oid FROM member WHERE (discord IS NULL AND mcid IS NULL)")
            .fetch_all(&db.pool)
            .await
            .context("")?;

        if !rows.is_empty() {
            send!(ctx, msg, format!("member no links: {:?}", rows));
        }

        // Checks if profile link lead to nonexistence profile
        let rows = sqlx::query!("SELECT oid FROM member WHERE \
                                (discord NOT NULL AND NOT EXISTS (SELECT 1 FROM discord WHERE id=member.discord AND mid=member.oid)) OR \
                                (mcid NOT NULL AND NOT EXISTS (SELECT 1 FROM wynn WHERE id=member.mcid AND mid=member.oid))")
            .fetch_all(&db.pool)
            .await
            .context("")?;

        if !rows.is_empty() {
            send!(ctx, msg, format!("member dangling link: {:?}", rows));
        }
    }

    {
        let db = db.read().await;
        // Checks if member link lead to nonexistence member
        let rows = sqlx::query!("SELECT id FROM discord WHERE \
                                 mid NOT NULL AND NOT EXISTS (SELECT 1 FROM member WHERE oid=discord.mid AND discord=discord.id)")
            .fetch_all(&db.pool)
            .await
            .context("")?;

        if !rows.is_empty() {
            send!(ctx, msg, format!("discord dangling mid: {:?}", rows));
        }
    }

    {
        let db = db.read().await;
        // Checks if member link lead to nonexistence member
        let rows = sqlx::query!("SELECT id FROM wynn WHERE \
                                 mid NOT NULL AND NOT EXISTS (SELECT 1 FROM member WHERE oid=wynn.mid AND mcid=wynn.id)")
            .fetch_all(&db.pool)
            .await
            .context("")?;

        if !rows.is_empty() {
            send!(ctx, msg, format!("wynn dangling mid: {:?}", rows));
        }

        // Checks if the value of `wynn.guild` is wrong
        let rows = sqlx::query!(
            "SELECT id FROM wynn WHERE \
                                 guild AND NOT EXISTS (SELECT 1 FROM guild WHERE id=wynn.id) OR \
                                 NOT guild AND EXISTS (SELECT 1 FROM guild WHERE id=wynn.id)"
        )
        .fetch_all(&db.pool)
        .await
        .context("")?;

        if !rows.is_empty() {
            send!(ctx, msg, format!("wynn invalid guild flag: {:?}", rows));
        }
    }

    ctx!(msg.edit(&ctx, |e| { e.content("done") }).await)?;

    Ok(())
}
