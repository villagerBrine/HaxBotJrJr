//! Config utilities
use std::fmt;
use std::str::FromStr;

use util::ioerr;

use crate::tag::{ChannelTag, Tag, TextChannelTag, UserTag};

/// All possible tags.
///
/// Useful if you want to parse any tag type from string.
/// ```
/// use std::str::FromStr;
/// use config::utils::Tags;
/// use config::tag::{ChannelTag, UserTag};
///
/// assert!(Tags::from_str("NoTrack").unwrap() == Tags::Channel(ChannelTag::NoTrack));
/// assert!(Tags::from_str("NoNickUpdate").unwrap() == Tags::User(UserTag::NoNickUpdate));
/// assert!(Tags::from_str("What").is_err());
/// ```
/// Because there is no overlap between the sets of parse-able strings of each tag types, the
/// [`FromStr`] of [`Tags`] works by simply trying to parse the string into each tag types using
/// their own [`FromStr`], and return the value if it succeeds.
///
/// [`FromStr`]: std::str::FromStr
#[derive(Debug, Eq, Hash, Clone, PartialEq)]
pub enum Tags {
    Channel(ChannelTag),
    TextChannel(TextChannelTag),
    User(UserTag),
}

impl Tag for Tags {
    fn describe(&self) -> &str {
        match self {
            Self::Channel(t) => t.describe(),
            Self::TextChannel(t) => t.describe(),
            Self::User(t) => t.describe(),
        }
    }
}

impl FromStr for Tags {
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

impl fmt::Display for Tags {
    /// Alias to the inner tag value's [`fmt`].
    ///
    /// [`fmt`]: std::fmt::Display::fmt
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Channel(t) => t.fmt(f),
            Self::TextChannel(t) => t.fmt(f),
            Self::User(t) => t.fmt(f),
        }
    }
}
