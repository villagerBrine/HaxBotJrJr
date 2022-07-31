//! Models for the wynn table
use crate::model::member::MemberId;

pub type McId = String;

#[derive(Debug)]
/// Wynn table model with database primitives.
/// Use this to query entire wynn profile from database, and convert it to `WynnProfile` with more
/// convenient field values.
pub struct WynnProfileRow {
    pub id: McId,
    pub mid: Option<MemberId>,
    pub guild: i64,
    pub ign: String,
    pub emerald: i64,
    pub emerald_week: i64,
    pub activity: i64,
    pub activity_week: i64,
}

#[derive(Debug)]
/// Wynn table model.
/// This can't be used to query entire wynn profile from database, instead query one using
/// `WynnProfileRow`, and then convert it to `WynnProfile`.
pub struct WynnProfile {
    pub id: McId,
    pub mid: Option<MemberId>,
    pub guild: bool,
    pub ign: String,
    pub emerald: i64,
    pub emerald_week: i64,
    pub activity: i64,
    pub activity_week: i64,
}

impl WynnProfile {
    /// Convert from `WynnProfileRow`
    pub fn from_row(row: WynnProfileRow) -> WynnProfile {
        WynnProfile {
            id: row.id,
            mid: row.mid,
            guild: row.guild > 0,
            ign: row.ign,
            emerald: row.emerald,
            emerald_week: row.emerald_week,
            activity: row.activity,
            activity_week: row.activity_week,
        }
    }
}
