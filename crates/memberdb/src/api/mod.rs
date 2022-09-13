//! Function for interacting with the database
pub mod fetch;
pub mod table;
pub mod update;

use anyhow::Result;

use crate::DB;

pub async fn check_integrity(db: &DB) -> Result<Vec<String>> {
    let mut issues = Vec::new();

    let rows = sqlx::query!(
        "SELECT oid FROM member WHERE 
            (discord NOT NULL AND mcid NOT NULL AND type!='full') OR 
            (discord NOT NULL AND mcid IS NULL AND type!='discord') OR
            (discord IS NULL AND mcid NOT NULL AND 
            NOT (SELECT guild FROM wynn WHERE id=member.mcid) AND type!='wynn') OR 
            (discord IS NULL AND mcid NOT NULL AND 
            (SELECT guild FROM wynn WHERE id=member.mcid) AND type!='guild')"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Wrong member type: {:?}", rows));
    }

    let rows = sqlx::query!("SELECT oid FROM member WHERE (discord IS NULL AND mcid IS NULL)")
        .fetch_all(&db.pool)
        .await?;
    if !rows.is_empty() {
        issues.push(format!("Empty member: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT oid FROM member WHERE 
            (discord NOT NULL AND NOT EXISTS (SELECT 1 FROM discord WHERE id=member.discord AND mid=member.oid)) OR 
            (mcid NOT NULL AND NOT EXISTS (SELECT 1 FROM wynn WHERE id=member.mcid AND mid=member.oid))")
        .fetch_all(&db.pool)
        .await?;
    if !rows.is_empty() {
        issues.push(format!("Bad profile link: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT id FROM discord WHERE
            mid NOT NULL AND NOT EXISTS (SELECT 1 FROM member WHERE oid=discord.mid AND discord=discord.id)"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Bad member link from discord: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT id FROM wynn WHERE 
            mid NOT NULL AND NOT EXISTS (SELECT 1 FROM member WHERE oid=wynn.mid AND mcid=wynn.id)"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Bad member link from wynn: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT id FROM wynn WHERE
            guild AND NOT EXISTS (SELECT 1 FROM guild WHERE id=wynn.id)"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Missing guild profile: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT id FROM guild WHERE
            NOT EXISTS (SELECT 1 FROM wynn WHERE id=guild.id)"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Missing wynn profile: {:?}", rows));
    }

    let rows = sqlx::query!(
        "SELECT id FROM wynn WHERE
            guild AND EXISTS (SELECT 1 FROM guild WHERE id=wynn.id) 
                AND NOT EXISTS (SELECT 1 FROM guild WHERE mid=wynn.mid)"
    )
    .fetch_all(&db.pool)
    .await?;
    if !rows.is_empty() {
        issues.push(format!("Mismatched member link between wynn & guild: {:?}", rows));
    }

    Ok(issues)
}
