//! String related functions
use anyhow::{bail, Result};
use num_format::{Locale, ToFormattedString};

use crate::{div_rem, ok};

/// Join an iterator over [`&Display`] into string with ", "
/// ```
/// # use util::string::str_join_iter;
/// use std::fmt;
///
/// struct Num(i64);
///
/// impl fmt::Display for Num {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
///         write!(f, "{}", self.0)
///     }
/// }
///
/// let list = vec![Num(0), Num(1), Num(2)];
/// let string = str_join_iter(list.iter());
/// assert!(string == "0, 1, 2");
/// ```
///
/// [`&Display`]: std::fmt::Display
pub fn str_join_iter<'a, I, T>(iter: I) -> String
where
    I: Iterator<Item = &'a T>,
    T: std::fmt::Display + 'a,
{
    let v: Vec<String> = iter.map(|t| t.to_string()).collect();
    v.join(", ")
}

/// Format seconds into user friendly string
///
/// The highest unit of time is week.
/// ```
/// # use util::string::fmt_second;
/// assert!(fmt_second(12) == "12s");
/// assert!(fmt_second(70) == "1m 10s");
/// println!("{}", fmt_second(1000000000));
/// assert!(fmt_second(1000000000) == "1653w 3d 1h 46m 40s");
/// ```
pub fn fmt_second(seconds: i64) -> String {
    if seconds == 0 {
        return "0s".to_string();
    }

    let (minuts, seconds) = div_rem!(seconds, 60);
    let (hours, minuts) = div_rem!(minuts, 60);
    let (days, hours) = div_rem!(hours, 24);
    let (weeks, days) = div_rem!(days, 7);

    let mut s = String::new();
    if weeks > 0 {
        s.push_str(&weeks.to_string());
        s.push_str("w ");
    }
    if days > 0 {
        s.push_str(&days.to_string());
        s.push_str("d ");
    }
    if hours > 0 {
        s.push_str(&hours.to_string());
        s.push_str("h ");
    }
    if minuts > 0 {
        s.push_str(&minuts.to_string());
        s.push_str("m ");
    }
    if seconds > 0 {
        s.push_str(&seconds.to_string());
        s.push('s');
    }

    s
}

/// Parse a string into seconds.
///
/// The string need to be in the format of `(whole integer)(time unit)`.
/// Time unit can be the following:
/// - `s`: seconds
/// - `m`: minutes
/// - `h`: hours
/// - `d`: days
/// - `w`: weeks
///
/// The integer can contain `,` separators, ex: "12,000h".
/// Multiple expressions can be chained together: "10w2d21h".
/// ```
/// # use util::string::parse_second;
/// assert!(parse_second("12s").unwrap() == 12);
/// assert!(parse_second("12,000h").unwrap() == 43200000);
/// assert!(parse_second("10w2d21h").unwrap() == 6296400);
/// ```
///
/// # Errors
/// Returns [`Result::Err`] is the given string is empty or isn't in a valid format
pub fn parse_second(s: &str) -> Result<u64> {
    if s.is_empty() {
        bail!("Empty")
    }

    let mut seconds = 0;
    let mut search_buffer = String::new();
    for c in s.chars() {
        // ignore ','
        if c == ',' {
            continue;
        }

        if "0123456789".contains(c) {
            search_buffer.push(c);
        } else {
            if search_buffer.is_empty() {
                bail!("Invalid format")
            }
            // Parse collected number
            let searched_num: u64 = ok!(search_buffer.parse(), bail!("Invalid number '{}'", search_buffer));
            search_buffer.clear();

            // Parse time unit
            let multiplier = match c {
                's' => 1,
                'm' => 60,
                'h' => 3600,
                'd' => 86400,
                'w' => 604800,
                _ => bail!("Unknown time unit '{}'", c),
            };
            seconds += searched_num * multiplier;
        }
    }

    Ok(seconds)
}

/// Format a number into String.
/// If `shorthand` is `true`, and number >= 1M, it is then formatted in shorthand up to billions.
/// ```
/// # use util::string::fmt_num;
/// assert!(fmt_num(10_000, false) == "10,000");
/// assert!(fmt_num(10_000, true) == "10,000");
/// assert!(fmt_num(12_345_000, false) == "12,345,000");
/// assert!(fmt_num(12_345_000, true) == "12.34M");
/// assert!(fmt_num(12_345_678_000_000, true) == "12345.67B");
/// ```
pub fn fmt_num(num: i64, shorthand: bool) -> String {
    if shorthand && num >= 1_000_000 {
        return if num >= 1_000_000_000 {
            let mut num = (num / 10_000_000) as f64;
            num /= 100.0;
            format!("{}B", num)
        } else {
            let mut num = (num / 10_000) as f64;
            num /= 100.0;
            format!("{}M", num)
        };
    }
    num.to_formatted_string(&Locale::en)
}

/// Parse a string into an integer.
///
/// The number can contain `,` separators, ex: "12,000".
/// The number can also be expressed in term of millions ("10m"), or billions ("10b").
/// ```
/// # use util::string::parse_num;
/// assert!(parse_num("12,000").unwrap() == 12000);
/// assert!(parse_num("10m").unwrap() == 10_000_000);
/// assert!(parse_num("1,234b").unwrap() == 1_234_000_000_000);
/// ```
///
/// # Errors
/// Returns [`Result::Err`] is the given string is empty or isn't a number.
pub fn parse_num(s: &str) -> Result<i64> {
    if s.is_empty() {
        bail!("empty")
    }

    let (s, multiplier) = if let Some(stripped) = s.strip_suffix('b') {
        (stripped, 1_000_000_000)
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, 1_000_000)
    } else {
        (s, 1)
    };

    let num: i64 = ok!(s.replace(',', "").parse(), bail!("Invalid number"));
    Ok(num * multiplier)
}

/// Deserialize content of file into `Option<...>`.
///
/// Takes the path to the json file, and an optional default value.
/// If the file doesn't exists and default value is provided, then returns that default value.
/// ```no_run
/// # use util::read_json;
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct Cache;
///
/// let cache: Option<Cache> = read_json!("cache.json");
/// let user_ids: Option<Vec<i64>> = read_json!("users.json", Vec::new());
/// ```
#[macro_export]
macro_rules! read_json {
    ($path:expr) => {
        match ::std::fs::read_to_string($path) {
            Ok(s) => match ::serde_json::from_str(&s) {
                Ok(json) => Some(json),
                Err(why) => {
                    ::tracing::error!("Failed to parse json file '{}': {:#}", $path, why);
                    None
                }
            },
            Err(why) => {
                ::tracing::error!("Failed to open file '{}': {:#}", $path, why);
                None
            }
        }
    };
    ($path:expr, $default:expr) => {
        match ::std::fs::read_to_string($path) {
            Ok(s) => match ::serde_json::from_str(&s) {
                Ok(json) => Some(json),
                Err(why) => {
                    ::tracing::error!("Failed to parse json file '{}': {:#}", $path, why);
                    None
                }
            },
            Err(why) => {
                if let ::std::io::ErrorKind::NotFound = why.kind() {
                    Some($default)
                } else {
                    ::tracing::error!("Failed to open file '{}': {:#}", $path, why);
                    None
                }
            }
        }
    };
}

/// Serialize a value into String and write to file
/// ```no_run
/// # use util::write_json;
/// let data = vec![1, 2, 3];
/// write_json!("data", &data, "user data");
/// ```
/// This macro takes a string that describes the value, and is used in error logging.
#[macro_export]
macro_rules! write_json {
    ($path:expr, $data:expr, $ctx:expr) => {
        match ::serde_json::to_string($data) {
            Ok(s) => match ::std::fs::write($path, s) {
                Ok(_) => {}
                Err(why) => ::tracing::error!("Failed to save {} to {}: {}", $ctx, $path, why),
            },
            Err(why) => ::tracing::error!("Failed to covert {} to string: {}", $ctx, why),
        }
    };
}
