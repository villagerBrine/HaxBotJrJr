//! Models for the discord table
use std::fmt;
use std::io;

use anyhow::Result;

use util::ioerr;

use crate::model::member::MemberId;

#[derive(sqlx::Type, Debug, Clone, Copy, PartialEq, Eq)]
#[sqlx(transparent)]
pub struct DiscordId(pub i64);

impl fmt::Display for DiscordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<u64> for DiscordId {
    type Error = io::Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match i64::try_from(value) {
            Ok(id) => Ok(Self(id)),
            Err(why) => ioerr!("failed to convert u64 to DiscordId: {:#}", why),
        }
    }
}

pub struct DiscordProfileRow {
    pub id: i64,
    pub mid: Option<i64>,
    pub message: i64,
    pub message_week: i64,
    pub image: i64,
    pub reaction: i64,
    pub voice: i64,
    pub voice_week: i64,
    pub activity: i64,
}

#[derive(Debug)]
/// Discord table model
pub struct DiscordProfile {
    pub id: DiscordId,
    pub mid: Option<MemberId>,
    pub message: i64,
    pub message_week: i64,
    pub image: i64,
    pub reaction: i64,
    pub voice: i64,
    pub voice_week: i64,
    pub activity: i64,
}

impl DiscordProfile {
    /// Convert from `MemberRow`
    pub fn from_row(row: DiscordProfileRow) -> Result<DiscordProfile> {
        Ok(DiscordProfile {
            id: DiscordId(row.id),
            mid: row.mid.map(|mid| MemberId(mid)),
            message: row.message,
            message_week: row.message_week,
            image: row.image,
            reaction: row.reaction,
            voice: row.voice,
            voice_week: row.voice_week,
            activity: row.activity,
        })
    }
}
