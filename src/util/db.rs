//! Utility function for interacting with `memberdb`
use serenity::model::id::UserId;
use tokio::sync::RwLock;

use memberdb::model::member::MemberId;
use memberdb::DB;
use util::{ctx, ok, some2};

/// Given discord id and mc id, return their linked member ids.
pub async fn get_profile_mids(db: &RwLock<DB>, discord_id: i64, mcid: &str) -> (Option<i64>, Option<i64>) {
    let db = db.read().await;
    let mid1 = ok!(ctx!(memberdb::get_wynn_mid(&db, mcid).await, "Failed to get wynn mid"), None);
    let mid2 = ok!(ctx!(memberdb::get_discord_mid(&db, discord_id).await, "Failed to get discord mid"), None);
    (mid1, mid2)
}

#[derive(Debug)]
/// Discord user id or mcid
pub enum TargetId {
    Discord(UserId),
    Wynn(String),
}

impl TargetId {
    /// Get linked member id
    pub async fn get_mid(&self, db: &DB) -> Option<MemberId> {
        match self {
            Self::Discord(id) => {
                let id = ok!(i64::try_from(*id), return None);
                some2!(memberdb::get_discord_mid(&db, id).await)
            }
            Self::Wynn(id) => some2!(memberdb::get_wynn_mid(&db, &id).await),
        }
    }
}
