use serenity::model::id::UserId;
use tokio::sync::RwLock;

use memberdb::error::DBError;
use memberdb::member::{MemberId, MemberType, ProfileType};
use memberdb::DB;
use util::{ctx, ok, some, some2};

/// Given discord id and mc id, return their linked member ids.
pub async fn get_profile_mids(db: &RwLock<DB>, discord_id: i64, mcid: &str) -> (Option<i64>, Option<i64>) {
    let db = db.read().await;
    let mid1 = ok!(ctx!(memberdb::get_wynn_mid(&db, mcid).await, "Failed to get wynn mid"), None);
    let mid2 = ok!(ctx!(memberdb::get_discord_mid(&db, discord_id).await, "Failed to get discord mid"), None);
    (mid1, mid2)
}

#[derive(Debug)]
pub enum TargetId {
    Discord(UserId),
    Wynn(String),
}

impl TargetId {
    pub async fn get_mid(&self, db: &DB) -> Option<MemberId> {
        match self {
            Self::Discord(id) => {
                let id = some!(memberdb::utils::from_user_id(*id), return None);
                some2!(memberdb::get_discord_mid(&db, id).await)
            }
            Self::Wynn(id) => some2!(memberdb::get_wynn_mid(&db, &id).await),
        }
    }
}

pub fn display_db_error(err: &DBError) -> &'static str {
    match err {
        DBError::MemberAlreadyExist(_) => "Specified member already exists",
        DBError::WrongMemberType(ty) => match ty {
            MemberType::Full => "This command can't be used on a full member",
            MemberType::DiscordPartial => "This command can't be used on a discord partial member",
            MemberType::GuildPartial => "This command can't be used on a guild partial member",
            MemberType::WynnPartial => "This command can't be used on a wynn partial member",
        },
        DBError::LinkOverride(ty, _) => match ty {
            ProfileType::Discord => "Discord user is already linked to another member",
            ProfileType::Wynn => "Mc account is already linked to another member",
            ProfileType::Guild => unreachable!(),
        },
    }
}
