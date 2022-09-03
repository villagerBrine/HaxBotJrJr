//! Functions that fetches multiple rows from database
use std::cmp::Ordering;

use anyhow::Result;
use serenity::client::Cache;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::model::discord::DiscordId;
use crate::query::{
    Column, Filter, MemberName, QueryAction, QueryBuilder, SelectAction, Selectable, Sort, Stat,
};
use crate::DB;

/// Return all members as list with optional filter applied.
/// Each member is represented as a list with following structure: [ign, discord name, member rank]
/// If a field doesn't exists, an empty string is used.
pub async fn list_members(cache: &Cache, db: &DB, filters: &Vec<Filter>) -> Result<Vec<Vec<String>>> {
    let mut query = QueryBuilder::new();
    query
        .with(&Column::WIgn)
        .with(&Column::MDiscord)
        .with(&Column::MRank)
        .with(&Sort::Asc(Column::WIgn));

    for filter in filters {
        query.with(filter);
    }
    let query = query.build();

    let query = sqlx::query(&query).map(|r: SqliteRow| {
        vec![
            // ign
            Column::WIgn.get_formatted(&r, &cache),
            // discord name
            match r
                .get::<Option<DiscordId>, &str>("discord")
                .map(|id| crate::utils::to_user(cache, id))
            {
                Some(Some(u)) => format!("{}#{}", u.name, u.discriminator),
                _ => String::new(),
            },
            // member rank
            Column::MRank.get_formatted(&r, &cache),
        ]
    });
    Ok(query.fetch_all(&db.pool).await?)
}

/// Get list of all ign that is associated with a member.
pub async fn list_igns(db: &DB) -> Result<Vec<String>> {
    Ok(sqlx::query!("SELECT ign FROM wynn WHERE mid NOT NULL")
        .map(|r| r.ign)
        .fetch_all(&db.pool)
        .await?)
}

/// Return a stat leaderboard and its heading.
///
/// The stat leaderboard can be applied with a filter.
/// Each row contains following items: [lb rank, name, stat val].
/// The name field is that member's ign, if not exist, their discord name is used.
///
/// if `no_zero` is true, then rows with stat val of 0 won't be included.
pub async fn stat_leaderboard(
    cache: &Cache, db: &DB, stat: &Stat, filters: &Vec<Filter>,
) -> Result<(Vec<Vec<String>>, Vec<String>)> {
    let stat_col = stat.to_column();
    let mut query = QueryBuilder::new();
    query.with(stat).with(&Sort::Desc(stat_col.clone())).with(&MemberName);

    let zero_filter = Filter::Stat(stat.clone(), 0, Ordering::Equal);
    let mut has_zero_filter = false;
    for filter in filters {
        if *filter == zero_filter {
            has_zero_filter = true;
        }
        query.with(filter);
    }

    // Only filter out entries with stat value of 0 if there isn't a filter that is specifically
    // looking for stat value of 0.
    if !has_zero_filter {
        query.filter(stat_col.get_ident().to_string());
    }
    query.with(&stat_col.profile().unwrap());

    let query = query.build_lb("r");

    let query = sqlx::query(&query).map(|r: SqliteRow| {
        let name = MemberName.get_formatted(&r, &cache);
        let lb_rank = r.get::<i64, _>("r");
        let stat_val = stat_col.get_formatted(&r, &cache);
        vec![lb_rank.to_string(), name, stat_val]
    });
    let result = query.fetch_all(&db.pool).await?;
    let header = vec![String::from("#"), String::from("name"), stat_col.get_table_name().to_string()];

    Ok((result, header))
}

/// Fetch values from the database by specifying what columns to select, and actions (like
/// filtering and ordering) to apply.
pub async fn make_table(
    cache: &Cache, db: &DB, cols: &Vec<impl Selectable>, actions: &Vec<impl QueryAction>,
) -> Result<(Vec<Vec<String>>, Vec<String>)> {
    let mut query = QueryBuilder::new();
    for col in cols {
        query.with(col);
    }
    for action in actions {
        query.with(action);
    }
    let query = query.build_lb("r");

    let query = sqlx::query(&query).map(|r: SqliteRow| {
        let rank = r.get::<i64, _>("r");

        let mut row = Vec::with_capacity(cols.len() + 1);
        row.push(rank.to_string());
        for col in cols {
            row.push(col.get_formatted(&r, &cache));
        }
        row
    });
    let result = query.fetch_all(&db.pool).await?;

    let mut header = Vec::with_capacity(cols.len() + 1);
    header.push(String::from("#"));
    for col in cols {
        header.push(col.get_table_name().to_string());
    }

    Ok((result, header))
}
