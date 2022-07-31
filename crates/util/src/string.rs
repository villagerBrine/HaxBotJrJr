//! String related functions
use num_format::{Locale, ToFormattedString};

/// Join an iterator over &Display into String
pub fn str_list_iter<'a, I, T>(iter: I) -> String
where
    I: Iterator<Item = &'a T>,
    T: std::fmt::Display + 'a,
{
    let v: Vec<String> = iter.map(|t| t.to_string()).collect();
    v.join(", ")
}

/// Format seconds into user friendly string
pub fn fmt_second(seconds: i64) -> String {
    if seconds == 0 {
        return "0s".to_string();
    }

    let (minuts, seconds) = crate::div_rem!(seconds, 60);
    let (hours, minuts) = crate::div_rem!(minuts, 60);
    let (days, hours) = crate::div_rem!(hours, 24);
    let (weeks, days) = crate::div_rem!(days, 7);

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
        s.push_str("s ");
    }

    s
}

/// Format a number into String, and if number >= 1K, it is formatted in shorthand up to billions
pub fn fmt_num(num: i64, shorthand: bool) -> String {
    if shorthand && num >= 1_000_000 {
        return if num >= 1_000_000_000 {
            let mut num = (num / 10_000_000) as f64;
            num = num / 100.0;
            format!("{}B", num)
        } else {
            let mut num = (num / 10_000) as f64;
            num = num / 100.0;
            format!("{}K", num)
        };
    }
    num.to_formatted_string(&Locale::en)
}

#[macro_export]
/// Deserialize content of file into type
macro_rules! read_json {
    ($path:expr) => {
        match std::fs::read_to_string($path) {
            Ok(s) => match serde_json::from_str(&s) {
                Ok(json) => Some(json),
                Err(why) => {
                    tracing::error!("Failed to parse json file '{}': {:#}", $path, why);
                    None
                }
            },
            Err(why) => {
                tracing::error!("Failed to open file '{}': {:#}", $path, why);
                None
            }
        }
    };
    ($path:expr, $default:expr) => {
        match std::fs::read_to_string($path) {
            Ok(s) => match serde_json::from_str(&s) {
                Ok(json) => Some(json),
                Err(why) => {
                    tracing::error!("Failed to parse json file '{}': {:#}", $path, why);
                    None
                }
            },
            Err(why) => {
                if let std::io::ErrorKind::NotFound = why.kind() {
                    Some($default)
                } else {
                    tracing::error!("Failed to open file '{}': {:#}", $path, why);
                    None
                }
            }
        }
    };
}

#[macro_export]
/// Serialize a type into String and write to file
macro_rules! write_json {
    ($path:expr, $data:expr, $ctx:expr) => {
        match serde_json::to_string($data) {
            Ok(s) => match std::fs::write($path, s) {
                Ok(_) => {}
                Err(why) => tracing::error!("Failed to save {} to {}: {}", $ctx, $path, why),
            },
            Err(why) => tracing::error!("Failed to covert {} to string: {}", $ctx, why),
        }
    };
}
