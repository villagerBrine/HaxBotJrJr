//! Utility functions
use std::time::Duration;

use backoff::ExponentialBackoffBuilder;
use reqwest::{Client, Response};
use tracing::{error, warn};

use util::some;

/// Checks if given string is valid mc ign
///
/// For a string to be a valid mc ign, following properties are needed:
/// 1. Btween 3 and 16 characters long.
/// 2. Contains only alphabets, numbers, or underscore.
pub fn is_valid_ign(ign: &str) -> bool {
    if let 3..=16 = ign.len() {
        for ch in ign.chars() {
            if !ch.is_alphanumeric() && ch != '_' {
                return false;
            }
        }
    } else {
        return false;
    }
    true
}

/// Return a dashed form of given mcid
/// ```
/// use wynn::utils::id_dashed;
/// assert!(id_dashed("12345678123412341234123456789abc") == Some("12345678-1234-1234-1234-123456789abc".to_string()))
/// ```
pub fn id_dashed(id: &str) -> Option<String> {
    Some(
        [
            some!(id.get(0..8), return None),
            some!(id.get(8..12), return None),
            some!(id.get(12..16), return None),
            some!(id.get(16..20), return None),
            some!(id.get(20..32), return None),
        ]
        .join("-"),
    )
}

/// Make an api request with exponential backoff
///
/// # Errors
/// Returns [`reqwest::Error`] if something went wrong while sending request.
pub async fn request(
    client: &Client, max_interal: u64, url: &str, ctx: &str,
) -> Result<Response, reqwest::Error> {
    let backoff = ExponentialBackoffBuilder::default()
        .with_max_interval(Duration::from_secs(max_interal))
        .build();
    backoff::future::retry(backoff, || async {
        let result = client.get(url).send().await;
        if let Err(why) = &result {
            request_error_log(why, ctx);
        }
        Ok(result?)
    })
    .await
}

/// Logs [`reqwest::Error`]
pub fn request_error_log(err: &reqwest::Error, ctx: &str) {
    if err.is_timeout() {
        warn!("Timeout when requesting {}: {}", ctx, err);
    } else if err.is_status() {
        error!("Received error status when requesting {}: {}", ctx, err);
    } else if err.is_request() {
        error!("Requesting {} failed: {}", ctx, err);
    } else if err.is_connect() {
        error!("Failed to connect when requesting {}: {}", ctx, err);
    } else {
        error!("Error when requesting {}: {}", ctx, err);
    }
}
