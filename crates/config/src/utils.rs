//! Config utilities
use std::fmt;
use std::str::FromStr;

use util::ioerr;

use crate::tag::{ChannelTag, Tag, TextChannelTag, UserTag};

#[derive(Debug, Eq, Hash, Clone, PartialEq)]
/// Union of different tag types.
/// Can be used to convert a string to any tag.
pub enum TagWrap {
    Channel(ChannelTag),
    TextChannel(TextChannelTag),
    User(UserTag),
}

impl Tag for TagWrap {
    fn describe(&self) -> &str {
        match self {
            Self::Channel(t) => t.describe(),
            Self::TextChannel(t) => t.describe(),
            Self::User(t) => t.describe(),
        }
    }
}

impl FromStr for TagWrap {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(tag) = ChannelTag::from_str(s) {
            return Ok(Self::Channel(tag));
        }
        if let Ok(tag) = TextChannelTag::from_str(s) {
            return Ok(Self::TextChannel(tag));
        }
        if let Ok(tag) = UserTag::from_str(s) {
            return Ok(Self::User(tag));
        }
        ioerr!("Failed to convert from '{}' to TagWrap", s)
    }
}

impl fmt::Display for TagWrap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Channel(t) => t.fmt(f),
            Self::TextChannel(t) => t.fmt(f),
            Self::User(t) => t.fmt(f),
        }
    }
}
