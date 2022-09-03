//! Functions for communication with Mojang/Wynncraft API
#[warn(missing_docs, missing_debug_implementations)]
pub mod cache;
pub mod error;
pub mod events;
pub mod loops;
pub mod model;
pub mod utils;

use anyhow::{bail, Context, Result};
use reqwest::Client;

use util::some;

use crate::error::IdDashingError;
use crate::model::{MojangIdResponse, MojangIgnResponse};

/// Get an ign's corresponding mcid via Mojang API.
///
/// # Errors
/// Returns [`reqwest::Error`] if something went wrong while sending request or failing to parse
/// the API response.
/// Returns [`IdDashingError`] if unable to convert the received id into its dashed form.
/// Returns [`anyhow::Error`] if provided ign is invalid.
pub async fn get_id(client: &Client, ign: &str) -> Result<String> {
    if !crate::utils::is_valid_ign(ign) {
        bail!("Invalid ign");
    }

    let mut url = "https://api.mojang.com/users/profiles/minecraft/".to_string();
    url.push_str(ign);

    let resp = crate::utils::request(client, 2, &url, "mojang api for ign id")
        .await
        .context("failed to request mojang api for ign id")?;

    let resp = resp
        .json::<MojangIdResponse>()
        .await
        .context("failed to parse mojang ign id response from json")?;

    let id = crate::utils::id_dashed(&resp.id).ok_or(IdDashingError)?;
    Ok(id)
}

/// Get a player's ign via Mojang api.
///
/// # Errors
/// Returns [`reqwest::Error`] if something went wrong while sending request or failing to parse
/// the API response.
/// Returns [`anyhow::Error`] if the player's ign history is empty.
pub async fn get_ign(client: &Client, mcid: &str) -> Result<String> {
    let url = format!("https://api.mojang.com/user/profiles/{}/names", mcid);

    let resp = crate::utils::request(client, 2, &url, "mojang api for igns")
        .await
        .context("failed to request mojang api for ign")?;

    let mut resp = resp
        .json::<MojangIgnResponse>()
        .await
        .context("failed to parse mojang ign response from json")?;

    let name = some!(resp.pop(), bail!("name history is empty"));
    Ok(name.name)
}
