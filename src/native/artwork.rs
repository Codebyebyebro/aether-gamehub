use crate::storage;
use image::{ImageReader, GenericImageView};
use std::{fs, io::{Cursor, Read}, path::{Path, PathBuf}, hash::{Hash, Hasher}, time::Duration};
const MAX_BYTES: u64 = 16 * 1024 * 1024;

fn allowed_url(value: &str) -> Result<url::Url, String> {
    let u = url::Url::parse(value).map_err(|_| "Invalid artwork URL")?;
    if u.scheme() != "https" || !u.username().is_empty() || u.password().is_some() || u.port().is_some() || !["cdn.akamai.steamstatic.com", "shared.akamai.steamstatic.com", "cdn.cloudflare.steamstatic.com", "cdn2.steamgriddb.com", "cdn.steamgriddb.com"].contains(&u.host_str().unwrap_or("")) { return Err("Artwork must use an approved HTTPS Steam or SteamGridDB CDN".into()); }
    Ok(u)
}
pub fn validate_reference(value: &str) -> Result<(), String> {
    if value.is_empty() { return Ok(()); }
    if value.starts_with("https:") { allowed_url(value)?; return Ok(()); }
    let p = Path::new(value).canonicalize().map_err(|_| "Artwork file is missing")?;
    let managed = storage::root()?.join("covers").canonicalize().map_err(|e| e.to_string())?;
    if !p.starts_with(managed) || !p.is_file() { return Err("Artwork must be imported into Aether Hub’s covers folder".into()); }
    Ok(())
}
fn decode(bytes: &[u8]) -> Result<image::DynamicImage, String> {
    if bytes.len() as u64 > MAX_BYTES { return Err("Artwork exceeds the 16 MB size limit".into()); }
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default(); limits.max_image_width = Some(6000); limits.max_image_height = Some(6000); limits.max_alloc = Some(160 * 1024 * 1024); reader.limits(limits);
    reader.decode().map_err(|e| format!("Unsupported or oversized image: {e}"))
}
pub fn import(path: &str) -> Result<String, String> {
    let source = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
    if !source.is_file() || fs::metadata(&source).map_err(|e| e.to_string())?.len() > MAX_BYTES { return Err("Choose an image smaller than 16 MB".into()); }
    let ext = source.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
    if !["png", "jpg", "jpeg", "webp"].contains(&ext.as_str()) { return Err("Choose PNG, JPEG, or WebP artwork".into()); }
    let bytes = fs::read(&source).map_err(|e| e.to_string())?; let image = decode(&bytes)?;
    let dir = storage::root()?.join("covers"); let id = uuid::Uuid::new_v4();
    fs::write(dir.join(format!("{id}-original.{ext}")), &bytes).map_err(|e| e.to_string())?;
    let out = dir.join(format!("{id}.webp"));
    let (w,h) = image.dimensions();
    let thumb = if h > w { image.thumbnail(600,900) } else { image.thumbnail(1600,900) };
    thumb.save(&out).map_err(|e| e.to_string())?;
    Ok(out.to_string_lossy().into_owned())
}
pub fn resolve(value: &str) -> Result<String, String> {
    if !value.starts_with("https:") { validate_reference(value)?; return Ok(value.into()); }
    let url = allowed_url(value)?;
    let mut hash = std::collections::hash_map::DefaultHasher::new(); value.hash(&mut hash);
    let dir = storage::root()?.join("covers"); let out: PathBuf = dir.join(format!("remote-{:x}.webp", hash.finish()));
    if out.is_file() { return Ok(out.to_string_lossy().into_owned()); }
    let cache_size: u64 = fs::read_dir(&dir).map_err(|e| e.to_string())?.filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().starts_with("remote-")).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum();
    if cache_size > 256 * 1024 * 1024 { return Err("Artwork cache budget reached; remote image remains available online".into()); }
    let client = reqwest::blocking::Client::builder().timeout(Duration::from_secs(12)).redirect(reqwest::redirect::Policy::none()).build().map_err(|e| e.to_string())?;
    let response = client.get(url).send().map_err(|_| "Artwork is not currently available online")?.error_for_status().map_err(|_| "Artwork is unavailable")?;
    if response.content_length().is_some_and(|s| s > MAX_BYTES) { return Err("Remote artwork exceeds the size limit".into()); }
    let mut bytes = Vec::new(); response.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    let image = decode(&bytes)?; let (w,h) = image.dimensions();
    let thumb = if h > w { image.thumbnail(600,900) } else { image.thumbnail(1600,900) };
    let temp = dir.join(format!("{}.webp", uuid::Uuid::new_v4())); thumb.save(&temp).map_err(|e| e.to_string())?;
    if !out.exists() { if let Err(e) = fs::rename(&temp, &out) { let _ = fs::remove_file(&temp); return Err(e.to_string()); } } else { let _ = fs::remove_file(&temp); }
    Ok(out.to_string_lossy().into_owned())
}
