//! Functions and event loop used to communicate with Mojang/Wynncraft api
pub mod cache;
pub mod loops;
pub mod model;
pub mod utils;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;

use crate::model::{MojangIgnIdResponse, MojangIgnResponse};
use util::some;

/// Get an ign's corresponding mcid via Mojang api.
pub async fn get_ign_id(client: &Client, ign: &str) -> Result<String> {
    if !crate::utils::is_valid_ign(ign) {
        bail!("Invalid ign");
    }

    let mut url = "https://api.mojang.com/users/profiles/minecraft/".to_string();
    url.push_str(ign);

    let resp = crate::utils::request(client, 2, &url, "mojang api for ign id")
        .await
        .context("Failed to request mojang api for ign id")?;

    let resp = resp
        .json::<MojangIgnIdResponse>()
        .await
        .context("Failed to parse mojang ign id response from json")?;

    let id = crate::utils::id_dashed(&resp.id)
        .ok_or(anyhow!("Failed to parse mojang ign id response to dashed id"))?;
    Ok(id)
}

/// Get a player's ign history via Mojang api.
pub async fn get_ign(client: &Client, mcid: &str) -> Result<String> {
    let url = format!("https://api.mojang.com/user/profiles/{}/names", mcid);

    let resp = crate::utils::request(client, 2, &url, "mojang api for igns")
        .await
        .context("Failed to request mojang api for ign")?;

    let mut resp = resp
        .json::<MojangIgnResponse>()
        .await
        .context("Failed to parse mojang ign response from json")?;

    let name = some!(resp.pop(), bail!("Name history is empty"));
    Ok(name.name)
}
