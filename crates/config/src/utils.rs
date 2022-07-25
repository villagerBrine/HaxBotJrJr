use std::fmt;
use std::io;
use std::str::FromStr;

use crate::tag::{ChannelTag, Tag, UserTag};

#[derive(Debug, Eq, Hash, Clone, PartialEq)]
/// Union of different tag types.
/// Can be used to convert a string to tag using `from_str`
pub enum TagWrap {
    Channel(ChannelTag),
    User(UserTag),
}

impl Tag for TagWrap {
    fn describe(&self) -> String {
        match self {
            Self::Channel(t) => t.describe(),
            Self::User(t) => t.describe(),
        }
    }
}

impl FromStr for TagWrap {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(tag) = ChannelTag::from_str(s) {
            return Ok(Self::Channel(tag));
        }
        if let Ok(tag) = UserTag::from_str(s) {
            return Ok(Self::User(tag));
        }
        Err(io::Error::new(io::ErrorKind::Other, "Failed to convert from str to TagWrap"))
    }
}

impl fmt::Display for TagWrap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Channel(t) => t.fmt(f),
            Self::User(t) => t.fmt(f),
        }
    }
}
