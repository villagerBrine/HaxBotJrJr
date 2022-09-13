//! Dev util commands
use std::process::Command;

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
        let issues = memberdb::check_integrity(&db).await?;
        if !issues.is_empty() {
            send!(ctx, msg, issues.join("\n"));
        }
    }

    ctx!(msg.edit(&ctx, |e| { e.content("done") }).await)?;

    Ok(())
}
