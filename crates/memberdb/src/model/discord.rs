//! Models for the discord table
use crate::model::member::MemberId;

pub type DiscordId = i64;

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