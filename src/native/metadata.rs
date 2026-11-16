use serde::{Serialize, Deserialize};
use std::time::Duration;
#[derive(Deserialize)] struct Response<T> { success: bool, data: T }
#[derive(Deserialize)] struct Entry { id: u64, name: String }
#[derive(Deserialize)] struct Grid { id: u64, url: String }
#[derive(Serialize)] pub struct Match { id: u64, name: String, url: String }
pub fn search(title: String, key: String) -> Result<Vec<Match>, String> {
    if title.trim().is_empty() || title.len() > 640 || key.len() > 200 || key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric()) { return Err("Enter a game title and a valid SteamGridDB API key".into()); }
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(12)).redirect(reqwest::redirect::Policy::none()).build().map_err(|e| e.to_string())?;
    let mut url = url::Url::parse("https://www.steamgriddb.com/api/v2/search/autocomplete/").map_err(|e| e.to_string())?;
    url.path_segments_mut().map_err(|_| "Invalid API URL")?.pop_if_empty().push(&title);
    let response: Response<Vec<Entry>> = client.get(url).bearer_auth(&key).send().map_err(|_| "SteamGridDB is unavailable")?.error_for_status().map_err(|_| "SteamGridDB rejected the request. Check your API key or try later.")?.json().map_err(|_| "Invalid SteamGridDB response")?;
    if !response.success { return Err("SteamGridDB search did not succeed".into()); }
    let mut matches = vec![];
    for entry in response.data.into_iter().take(3) {
        let response: Response<Vec<Grid>> = client.get(format!("https://www.steamgriddb.com/api/v2/grids/game/{}", entry.id)).query(&[("dimensions", "600x900"), ("types", "static")]).bearer_auth(&key).send().map_err(|_| "Artwork request failed")?.error_for_status().map_err(|_| "Artwork request was rejected")?.json().map_err(|_| "Invalid artwork response")?;
        if response.success { for grid in response.data.into_iter().take(2) { if crate::artwork::validate_reference(&grid.url).is_ok() { matches.push(Match { id: grid.id, name: entry.name.clone(), url: grid.url }); } } }
    }
    Ok(matches)
}
