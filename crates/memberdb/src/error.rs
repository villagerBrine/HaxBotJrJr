use std::fmt;

use crate::model::member::{MemberId, MemberType, ProfileType};

#[derive(Debug)]
/// Database errors
pub enum DBError {
    MemberAlreadyExist(MemberId),
    WrongMemberType(MemberType),
    LinkOverride(ProfileType, MemberId),
}

impl fmt::Display for DBError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MemberAlreadyExist(id) => write!(f, "Member already exists with id {}", id),
            Self::WrongMemberType(ty) => write!(f, "Wrong member type '{:?}'", ty),
            Self::LinkOverride(ty, id) => {
                write!(f, "Attempts to override an existing link to '{}' in profile '{:?}'", id, ty)
            }
        }
    }
}
impl std::error::Error for DBError {}
