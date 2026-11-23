//! "Living colors": the one colour that best stands for a game's artwork, so the interface can take it on.
//!
//! The picture is shrunk, greys and near-black/near-white pixels are set aside, and the hue family with the most vivid
//! area wins; its pixels are averaged. Artwork with no real colour (black-and-white art) gives a calm grey instead.
//! The result is clamped into a range that works as a tint behind light and dark themes alike.
use image::DynamicImage;
use serde::Serialize;
use std::{collections::HashMap, sync::Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Rgb { pub r: u8, pub g: u8, pub b: u8 }

const BINS: usize = 36;

fn hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b); let min = r.min(g).min(b); let d = max - min;
    let h = if d == 0.0 { 0.0 } else if max == r { 60.0 * (((g - b) / d).rem_euclid(6.0)) } else if max == g { 60.0 * ((b - r) / d + 2.0) } else { 60.0 * ((r - g) / d + 4.0) };
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}
fn rgb(h: f32, s: f32, v: f32) -> Rgb {
    let c = v * s; let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs()); let m = v - c;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 { 0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x), 3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x) };
    let to = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb { r: to(r), g: to(g), b: to(b) }
}

/// The colour of a picture, ready to use as a tint.
pub fn of_image(image: &DynamicImage) -> Rgb {
    let small = image.thumbnail(64, 64).to_rgba8();
    let mut weight = [0f32; BINS]; let mut sums = [[0f32; 3]; BINS];
    let (mut all, mut count) = ([0f32; 3], 0f32);
    for p in small.pixels() {
        if p[3] < 128 { continue; }
        let (r, g, b) = (p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0);
        all[0] += r; all[1] += g; all[2] += b; count += 1.0;
        let (h, s, v) = hsv(r, g, b);
        if v < 0.15 || s < 0.22 || (v > 0.95 && s < 0.3) { continue; }
        // Vivid, bright pixels count most; a big dull area still loses to a smaller vivid one.
        let w = s * s * v;
        let bin = ((h / 360.0 * BINS as f32) as usize).min(BINS - 1);
        weight[bin] += w; sums[bin][0] += r * w; sums[bin][1] += g * w; sums[bin][2] += b * w;
    }
    if count == 0.0 { return Rgb { r: 128, g: 128, b: 140 }; }
    // A hue family spills into its neighbours, so each bin is scored with half of the bins beside it.
    let score = |i: usize| weight[i] + 0.5 * (weight[(i + BINS - 1) % BINS] + weight[(i + 1) % BINS]);
    let best = (0..BINS).max_by(|&a, &b| score(a).total_cmp(&score(b))).unwrap_or(0);
    let vivid_share = weight.iter().sum::<f32>() / count;
    let (h, s, v) = if vivid_share < 0.02 || weight[best] == 0.0 {
        // Black-and-white or washed-out art: its average tone, kept nearly grey.
        let (h, s, v) = hsv(all[0] / count, all[1] / count, all[2] / count);
        (h, s.min(0.12), v)
    } else {
        let w = weight[best];
        hsv(sums[best][0] / w, sums[best][1] / w, sums[best][2] / w)
    };
    // Never so dark it disappears on navy, never so bright or loud it fights with text on cream.
    let s = if s < 0.15 { s } else { s.clamp(0.35, 0.85) };
    rgb(h, s, v.clamp(0.45, 0.88))
}

static MEMO: Mutex<Option<HashMap<String, Rgb>>> = Mutex::new(None);

/// The colour for a cover or banner reference (an approved HTTPS address or a picture in the covers folder).
/// Remote artwork is downloaded into the same cache the library uses, so nothing is fetched twice.
pub fn of_reference(reference: &str) -> Result<Rgb, String> {
    if let Some(hit) = MEMO.lock().ok().and_then(|m| m.as_ref().and_then(|m| m.get(reference).copied())) { return Ok(hit); }
    let path = crate::artwork::resolve(reference)?;
    let mut reader = image::ImageReader::open(&path).map_err(|e| e.to_string())?.with_guessed_format().map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default(); limits.max_image_width = Some(6000); limits.max_image_height = Some(6000); limits.max_alloc = Some(160 * 1024 * 1024); reader.limits(limits);
    let picture = reader.decode().map_err(|e| format!("Artwork could not be read: {e}"))?;
    let colour = of_image(&picture);
    if let Ok(mut memo) = MEMO.lock() {
        let memo = memo.get_or_insert_with(HashMap::new);
        if memo.len() > 4000 { memo.clear(); }
        memo.insert(reference.to_owned(), colour);
    }
    Ok(colour)
}
/// Checks the request before anything is opened: the interface only ever asks about artwork references.
pub fn validate(reference: &str) -> Result<(), String> {
    if reference.is_empty() || reference.len() > 4096 { return Err("No artwork to take a colour from".into()); }
    crate::artwork::validate_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    fn picture(paint: impl Fn(u32, u32) -> [u8; 4]) -> DynamicImage { DynamicImage::ImageRgba8(RgbaImage::from_fn(60, 90, |x, y| Rgba(paint(x, y)))) }
    fn hue(c: Rgb) -> f32 { hsv(c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0).0 }
    fn sat(c: Rgb) -> f32 { hsv(c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0).1 }
    fn val(c: Rgb) -> f32 { hsv(c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0).2 }

    #[test] fn a_red_cover_gives_red() {
        let c = of_image(&picture(|_, _| [200, 30, 30, 255]));
        assert!(hue(c) < 10.0 || hue(c) > 350.0, "{c:?}");
        assert!(c.r > c.g && c.r > c.b);
    }
    #[test] fn a_small_vivid_area_beats_a_large_grey_one() {
        // Mostly dark grey, with a blue band: think of a dark cover with a glowing logo.
        let c = of_image(&picture(|_, y| if y < 20 { [40, 90, 230, 255] } else { [70, 70, 75, 255] }));
        assert!((200.0..250.0).contains(&hue(c)), "{c:?}");
    }
    #[test] fn the_hue_with_the_most_vivid_area_wins() {
        let c = of_image(&picture(|_, y| if y < 60 { [30, 180, 60, 255] } else { [220, 40, 160, 255] }));
        assert!((100.0..160.0).contains(&hue(c)), "green covers two thirds: {c:?}");
    }
    #[test] fn black_and_white_art_gives_a_calm_grey() {
        let c = of_image(&picture(|x, _| if x % 2 == 0 { [10, 10, 10, 255] } else { [240, 240, 240, 255] }));
        assert!(sat(c) <= 0.15, "{c:?}");
    }
    #[test] fn the_colour_always_works_as_a_tint() {
        for paint in [[0u8, 0, 0, 255], [255, 255, 255, 255], [5, 5, 60, 255], [255, 255, 0, 255], [0, 255, 0, 255]] {
            let c = of_image(&picture(|_, _| paint));
            assert!((0.44..=0.89).contains(&val(c)), "{paint:?} → {c:?} is too dark or too bright");
            assert!(sat(c) <= 0.86, "{paint:?} → {c:?} is too loud");
        }
    }
    #[test] fn a_transparent_picture_does_not_break() {
        assert_eq!(of_image(&picture(|_, _| [0, 0, 0, 0])), Rgb { r: 128, g: 128, b: 140 });
    }
    #[test] fn hsv_round_trips() {
        for (r, g, b) in [(255u8, 0u8, 0u8), (0, 255, 0), (0, 0, 255), (120, 60, 200), (250, 200, 10)] {
            let (h, s, v) = hsv(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
            let back = rgb(h, s, v);
            assert!((back.r as i32 - r as i32).abs() <= 1 && (back.g as i32 - g as i32).abs() <= 1 && (back.b as i32 - b as i32).abs() <= 1, "{r},{g},{b} → {back:?}");
        }
    }
    #[test] fn only_artwork_references_are_accepted() {
        assert!(validate("").is_err());
        assert!(validate("https://evil.example/x.png").is_err());
        assert!(validate("C:\\Windows\\System32\\x.png").is_err());
    }
    #[test] fn the_answer_has_the_names_the_interface_reads() {
        assert_eq!(serde_json::to_string(&Rgb { r: 1, g: 2, b: 3 }).unwrap(), r#"{"r":1,"g":2,"b":3}"#);
    }
}
