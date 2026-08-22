//! Go-compatible feed-icon processing.
//!
//! FluxBar feed icons are fetched from Miniflux as data URLs, normalized to a
//! square transparent PNG, and optionally augmented with a light rounded
//! background for dark menus.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ImageEncoder, Rgba, RgbaImage};

use crate::remote::RemoteInbox;

pub const DEFAULT_SIZE: u32 = 32;
const MEANINGFUL_TRANSPARENCY_RATIO: f64 = 0.10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackgroundMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl BackgroundMode {
    pub fn from_env_lists(
        always: &HashSet<i64>,
        never: &HashSet<i64>,
        feed_id: i64,
    ) -> BackgroundMode {
        if always.contains(&feed_id) {
            BackgroundMode::Always
        } else if never.contains(&feed_id) {
            BackgroundMode::Never
        } else {
            BackgroundMode::Auto
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppearanceAnalysis {
    pub mean_luminance: f64,
    pub dark_contrast: f64,
    pub low_contrast_ratio: f64,
    pub visible_coverage: f64,
    pub has_transparency: bool,
    pub transparent_ratio: f64,
    pub classified_dark: bool,
    pub background_mode: BackgroundMode,
    pub background_added: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconError {
    InvalidDataUrl,
    NotAnImage,
    NotBase64,
    DecodeFailed(String),
    EmptyIcon,
    InvalidSize,
    UnknownBackgroundMode(String),
}

impl std::fmt::Display for IconError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IconError::InvalidDataUrl => write!(f, "ungueltige Icon-Daten-URL"),
            IconError::NotAnImage => write!(f, "Icon ist kein Bild"),
            IconError::NotBase64 => write!(f, "Icon ist nicht base64-kodiert"),
            IconError::DecodeFailed(reason) => {
                write!(f, "Icon kann nicht dekodiert werden: {reason}")
            }
            IconError::EmptyIcon => write!(f, "Icon ist leer"),
            IconError::InvalidSize => write!(f, "Icon hat keine gueltige Groesse"),
            IconError::UnknownBackgroundMode(mode) => {
                write!(f, "unbekannter Hintergrundmodus {mode:?}")
            }
        }
    }
}

impl std::error::Error for IconError {}

pub fn decode_data_url(value: &str) -> Result<(String, Vec<u8>), IconError> {
    let value = value.trim();
    let (header, payload) = value.split_once(',').ok_or(IconError::InvalidDataUrl)?;
    if payload.is_empty() {
        return Err(IconError::InvalidDataUrl);
    }
    let header = header.trim();
    let header = header.strip_prefix("data:").unwrap_or(header);
    let mut parts = header.split(';');
    let media_type = parts
        .next()
        .ok_or(IconError::InvalidDataUrl)?
        .trim()
        .to_lowercase();
    if !media_type.starts_with("image/") {
        return Err(IconError::NotAnImage);
    }
    let mut base64_encoded = false;
    for part in parts {
        if part.trim().eq_ignore_ascii_case("base64") {
            base64_encoded = true;
        }
    }
    if !base64_encoded {
        return Err(IconError::NotBase64);
    }
    let decoded = base64_decode(payload).map_err(|e| IconError::DecodeFailed(e.to_string()))?;
    Ok((media_type, decoded))
}

fn base64_decode(payload: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(payload.trim())
}

pub fn normalize_data_url(value: &str, size: u32) -> Result<Vec<u8>, IconError> {
    let (media_type, data) = decode_data_url(value)?;
    normalize(&data, &media_type, size)
}

pub fn normalize(data: &[u8], media_type: &str, size: u32) -> Result<Vec<u8>, IconError> {
    if data.is_empty() {
        return Err(IconError::EmptyIcon);
    }
    let size = if size == 0 { DEFAULT_SIZE } else { size };
    let source = if is_svg(media_type, data) {
        rasterize_svg(data, size)?
    } else {
        decode_raster(data)?
    };
    resize_and_encode(&source, size)
}

fn is_svg(media_type: &str, data: &[u8]) -> bool {
    if media_type.to_lowercase().contains("svg") {
        return true;
    }
    let prefix = std::str::from_utf8(data).unwrap_or("").trim_start();
    prefix.starts_with("<svg") || prefix.starts_with("<?xml")
}

fn decode_raster(data: &[u8]) -> Result<RgbaImage, IconError> {
    Ok(image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| IconError::DecodeFailed(e.to_string()))?
        .decode()
        .map_err(|e| IconError::DecodeFailed(e.to_string()))?
        .to_rgba8())
}

fn rasterize_svg(data: &[u8], size: u32) -> Result<RgbaImage, IconError> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(data, &opt)
        .map_err(|e| IconError::DecodeFailed(e.to_string()))?;
    let target =
        resvg::usvg::Size::from_wh(size as f32, size as f32).ok_or(IconError::InvalidSize)?;
    let scaled = tree.size().scale_to(target);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).ok_or(IconError::InvalidSize)?;
    pixmap.fill(resvg::tiny_skia::Color::TRANSPARENT);
    let x = ((size as f32) - scaled.width()) / 2.0;
    let y = ((size as f32) - scaled.height()) / 2.0;
    let transform = resvg::tiny_skia::Transform::from_translate(x, y).pre_concat(
        resvg::tiny_skia::Transform::from_scale(
            scaled.width() / tree.size().width(),
            scaled.height() / tree.size().height(),
        ),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(rgba_image_from_pixmap(&pixmap))
}

fn rgba_image_from_pixmap(pixmap: &resvg::tiny_skia::Pixmap) -> RgbaImage {
    let data = pixmap.data();
    RgbaImage::from_raw(pixmap.width(), pixmap.height(), data.to_vec())
        .expect("pixmap dimensions match")
}

fn resize_and_encode(source: &RgbaImage, size: u32) -> Result<Vec<u8>, IconError> {
    let (src_w, src_h) = source.dimensions();
    if src_w == 0 || src_h == 0 {
        return Err(IconError::InvalidSize);
    }
    let scale = f64::min(size as f64 / src_w as f64, size as f64 / src_h as f64);
    let width = (src_w as f64 * scale).round().max(1.0) as u32;
    let height = (src_h as f64 * scale).round().max(1.0) as u32;
    let x = (size - width) / 2;
    let y = (size - height) / 2;
    let resized = image::imageops::resize(
        source,
        width,
        height,
        image::imageops::FilterType::CatmullRom,
    );
    let mut destination = RgbaImage::new(size, size);
    image::imageops::overlay(&mut destination, &resized, x as i64, y as i64);
    encode_png(&destination)
}

fn encode_png(source: &RgbaImage) -> Result<Vec<u8>, IconError> {
    let mut output = Vec::new();
    let encoder =
        PngEncoder::new_with_quality(&mut output, CompressionType::Fast, FilterType::Adaptive);
    encoder
        .write_image(
            source.as_raw(),
            source.width(),
            source.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| IconError::DecodeFailed(e.to_string()))?;
    Ok(output)
}

pub fn dark_mode_variant(data: &[u8], mode: BackgroundMode) -> Result<Option<Vec<u8>>, IconError> {
    let source = image::load_from_memory(data)
        .map_err(|e| IconError::DecodeFailed(format!("normalisiertes Icon analysieren: {e}")))?
        .to_rgba8();
    let mut analysis = analyze_appearance(&source);
    analysis.background_mode = mode;
    let mut add_background = analysis.classified_dark && analysis.has_transparency;
    match mode {
        BackgroundMode::Always => add_background = true,
        BackgroundMode::Never => add_background = false,
        BackgroundMode::Auto => {}
    }
    if !add_background {
        return Ok(None);
    }
    let (w, h) = source.dimensions();
    if w == 0 || h == 0 {
        return Err(IconError::InvalidSize);
    }
    let mut destination = RgbaImage::new(w, h);
    draw_rounded_surface(&mut destination);
    let padding = (w.min(h) / 12).max(2);
    let target_width = w.saturating_sub(2 * padding);
    let target_height = h.saturating_sub(2 * padding);
    if target_width > 0 && target_height > 0 {
        let resized = image::imageops::resize(
            &source,
            target_width,
            target_height,
            image::imageops::FilterType::CatmullRom,
        );
        image::imageops::overlay(&mut destination, &resized, padding as i64, padding as i64);
    }
    let variant = encode_png(&destination)
        .map_err(|e| IconError::DecodeFailed(format!("Dark-Mode-Icon kodieren: {e}")))?;
    Ok(Some(variant))
}

fn analyze_appearance(source: &RgbaImage) -> AppearanceAnalysis {
    let (width, height) = source.dimensions();
    let pixel_count = (width * height) as usize;
    if pixel_count == 0 {
        return AppearanceAnalysis::default();
    }
    let dark_background = linear_srgb(0.12);
    let mut weight_sum = 0.0;
    let mut luminance_sum = 0.0;
    let mut contrast_sum = 0.0;
    let mut low_contrast_weight = 0.0;
    let mut transparent_pixels = 0usize;
    for y in 0..height {
        for x in 0..width {
            let Rgba([r, g, b, a]) = *source.get_pixel(x, y);
            if a < 255 {
                transparent_pixels += 1;
            }
            if a == 0 {
                continue;
            }
            let alpha = a as f64 / 255.0;
            let red = (r as f64 / alpha).min(255.0);
            let green = (g as f64 / alpha).min(255.0);
            let blue = (b as f64 / alpha).min(255.0);
            let luminance = 0.2126 * linear_srgb(red)
                + 0.7152 * linear_srgb(green)
                + 0.0722 * linear_srgb(blue);
            let composited = alpha * luminance + (1.0 - alpha) * dark_background;
            let contrast = contrast_ratio(composited, dark_background);
            weight_sum += alpha;
            luminance_sum += luminance * alpha;
            contrast_sum += contrast * alpha;
            if contrast < 2.25 {
                low_contrast_weight += alpha;
            }
        }
    }
    let transparent_ratio = transparent_pixels as f64 / pixel_count as f64;
    let mut analysis = AppearanceAnalysis {
        has_transparency: transparent_ratio >= MEANINGFUL_TRANSPARENCY_RATIO,
        transparent_ratio,
        ..Default::default()
    };
    if weight_sum == 0.0 {
        return analysis;
    }
    analysis.mean_luminance = luminance_sum / weight_sum;
    analysis.dark_contrast = contrast_sum / weight_sum;
    analysis.low_contrast_ratio = low_contrast_weight / weight_sum;
    analysis.visible_coverage = weight_sum / pixel_count as f64;
    analysis.classified_dark = analysis.visible_coverage >= 0.01
        && analysis.mean_luminance <= 0.32
        && analysis.low_contrast_ratio >= 0.50;
    analysis
}

fn linear_srgb(value: f64) -> f64 {
    let value = value / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn contrast_ratio(first: f64, second: f64) -> f64 {
    let (lighter, darker) = if first > second {
        (first, second)
    } else {
        (second, first)
    };
    (lighter + 0.05) / (darker + 0.05)
}

fn draw_rounded_surface(destination: &mut RgbaImage) {
    let (w, h) = destination.dimensions();
    if w == 0 || h == 0 {
        return;
    }
    let inset = 1.0;
    let left = inset;
    let top = inset;
    let right = (w as f64) - inset;
    let bottom = (h as f64) - inset;
    let radius = ((w.min(h) as f64) * 0.20).max(3.0);
    let background = Rgba([242, 242, 242, 235]);
    const SAMPLES: u32 = 4;
    for y in 0..h {
        for x in 0..w {
            let mut inside = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let px = x as f64 + (sx as f64 + 0.5) / SAMPLES as f64;
                    let py = y as f64 + (sy as f64 + 0.5) / SAMPLES as f64;
                    if inside_rounded_rectangle(px, py, left, top, right, bottom, radius) {
                        inside += 1;
                    }
                }
            }
            if inside > 0 {
                let mut pixel = background;
                pixel[3] = (background[3] as u32 * inside / (SAMPLES * SAMPLES)) as u8;
                destination.put_pixel(x, y, pixel);
            }
        }
    }
}

fn inside_rounded_rectangle(
    x: f64,
    y: f64,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    radius: f64,
) -> bool {
    if x < left || x >= right || y < top || y >= bottom {
        return false;
    }
    let inner_left = left + radius;
    let inner_right = right - radius;
    let inner_top = top + radius;
    let inner_bottom = bottom - radius;
    if x >= inner_left && x < inner_right {
        return true;
    }
    if y >= inner_top && y < inner_bottom {
        return true;
    }
    let corner_x = if x < inner_left {
        inner_left
    } else {
        inner_right
    };
    let corner_y = if y < inner_top {
        inner_top
    } else {
        inner_bottom
    };
    let dx = x - corner_x;
    let dy = y - corner_y;
    dx * dx + dy * dy <= radius * radius
}

/// In-memory cache entry for one feed icon.
#[derive(Debug, Clone, Default)]
pub struct CachedIcon {
    pub regular: Vec<u8>,
    pub dark: Vec<u8>,
}

/// Go-compatible in-memory icon cache with single-flight concurrent-load
/// deduplication. Errors are not cached; each failure leaves the slot empty so
/// the next caller retries.
pub struct IconService {
    cache: Mutex<HashMap<i64, CachedIcon>>,
    loads: Mutex<HashMap<i64, Arc<LoadSlot>>>,
    background_always: HashSet<i64>,
    background_never: HashSet<i64>,
}

struct LoadSlot {
    ready: Mutex<bool>,
    condvar: Condvar,
    waiters: AtomicUsize,
}

struct LoadGuard<'a> {
    service: &'a IconService,
    feed_id: i64,
    slot: Arc<LoadSlot>,
}

impl Drop for LoadGuard<'_> {
    fn drop(&mut self) {
        self.service
            .loads
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.feed_id);
        *self.slot.ready.lock().unwrap_or_else(|p| p.into_inner()) = true;
        self.slot.condvar.notify_all();
    }
}

impl Default for IconService {
    fn default() -> Self {
        Self::new()
    }
}

impl IconService {
    /// Creates a new icon cache. Background-mode overrides are read from the
    /// same environment variables used by the Go core:
    /// `FLUXBAR_ICON_BACKGROUND_ALWAYS` and `FLUXBAR_ICON_BACKGROUND_NEVER`,
    /// each a comma-separated list of feed IDs.
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            loads: Mutex::new(HashMap::new()),
            background_always: feed_id_set(
                std::env::var("FLUXBAR_ICON_BACKGROUND_ALWAYS").unwrap_or_default(),
            ),
            background_never: feed_id_set(
                std::env::var("FLUXBAR_ICON_BACKGROUND_NEVER").unwrap_or_default(),
            ),
        }
    }

    /// Returns the regular and dark byte arrays for `feed_id`. Missing or
    /// unprocessable icons return empty byte arrays, matching Go behavior.
    pub fn feed_icon(&self, feed_id: i64, remote: &dyn RemoteInbox) -> CachedIcon {
        loop {
            {
                let cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(icon) = cache.get(&feed_id) {
                    return icon.clone();
                }
            }

            let slot = {
                let loads = self.loads.lock().unwrap_or_else(|p| p.into_inner());
                loads.get(&feed_id).cloned()
            };

            if let Some(slot) = slot {
                let mut ready = slot.ready.lock().unwrap_or_else(|p| p.into_inner());
                slot.waiters.fetch_add(1, Ordering::SeqCst);
                while !*ready {
                    ready = slot.condvar.wait(ready).unwrap_or_else(|p| p.into_inner());
                }
                slot.waiters.fetch_sub(1, Ordering::SeqCst);
                return self
                    .cache
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .get(&feed_id)
                    .cloned()
                    .unwrap_or_default();
            }

            let slot = Arc::new(LoadSlot {
                ready: Mutex::new(false),
                condvar: Condvar::new(),
                waiters: AtomicUsize::new(0),
            });
            {
                let mut loads = self.loads.lock().unwrap_or_else(|p| p.into_inner());
                if loads.contains_key(&feed_id) {
                    continue;
                }
                loads.insert(feed_id, slot.clone());
            }

            let _load_guard = LoadGuard {
                service: self,
                feed_id,
                slot,
            };
            let result = self.load_and_process(feed_id, remote);
            if let Ok(icon) = result {
                self.cache
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .insert(feed_id, icon.clone());
                return icon;
            }
            return CachedIcon::default();
        }
    }

    fn load_and_process(&self, feed_id: i64, remote: &dyn RemoteInbox) -> Result<CachedIcon, ()> {
        let data_url = match remote.icon_data_url(feed_id) {
            Ok(Some(url)) => url,
            Ok(None) | Err(_) => return Err(()),
        };

        let regular = match normalize_data_url(&data_url, DEFAULT_SIZE) {
            Ok(bytes) => bytes,
            Err(_) => return Err(()),
        };

        let mode = BackgroundMode::from_env_lists(
            &self.background_always,
            &self.background_never,
            feed_id,
        );
        let dark = match dark_mode_variant(&regular, mode) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => Vec::new(),
            Err(_) => Vec::new(),
        };

        Ok(CachedIcon { regular, dark })
    }
}

fn feed_id_set(value: String) -> HashSet<i64> {
    value
        .split(',')
        .filter_map(|part| part.trim().parse::<i64>().ok())
        .filter(|id| *id > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use image::codecs::png::PngEncoder;
    use image::{ImageEncoder, Rgba, RgbaImage};

    macro_rules! remote_stubs {
        () => {
            fn fetch_complete_selection(
                &self,
                _filter: &crate::remote::EntriesFilter,
            ) -> Result<(Vec<crate::remote::EntryDto>, i64), crate::remote::RemoteError> {
                Ok((Vec::new(), 0))
            }

            fn categories(
                &self,
            ) -> Result<Vec<crate::remote::CategoryDto>, crate::remote::RemoteError> {
                Ok(Vec::new())
            }

            fn feeds(&self) -> Result<Vec<crate::remote::FeedDto>, crate::remote::RemoteError> {
                Ok(Vec::new())
            }

            fn unread_counters(
                &self,
            ) -> Result<crate::remote::FeedCountersDto, crate::remote::RemoteError> {
                Ok(crate::remote::FeedCountersDto {
                    unreads: HashMap::new(),
                })
            }

            fn starred_total(&self) -> Result<i64, crate::remote::RemoteError> {
                Ok(0)
            }

            fn set_read_batch(
                &self,
                _entry_ids: &[i64],
                _read: bool,
            ) -> Result<(), crate::remote::RemoteError> {
                Ok(())
            }

            fn entry_starred(&self, _entry_id: i64) -> Result<bool, crate::remote::RemoteError> {
                Ok(false)
            }

            fn toggle_starred(&self, _entry_id: i64) -> Result<(), crate::remote::RemoteError> {
                Ok(())
            }
        };
    }

    fn encode_test_png(source: &RgbaImage) -> Vec<u8> {
        let mut output = Vec::new();
        let encoder = PngEncoder::new(&mut output);
        encoder
            .write_image(
                source.as_raw(),
                source.width(),
                source.height(),
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        output
    }

    fn transparent_icon_png(foreground: Rgba<u8>) -> Vec<u8> {
        let mut source = RgbaImage::new(32, 32);
        for y in 6..26 {
            for x in 6..26 {
                source.put_pixel(x, y, foreground);
            }
        }
        encode_test_png(&source)
    }

    fn opaque_icon_png(foreground: Rgba<u8>) -> Vec<u8> {
        let mut source = RgbaImage::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                source.put_pixel(x, y, foreground);
            }
        }
        encode_test_png(&source)
    }

    #[test]
    fn decode_data_url_accepts_with_or_without_scheme() {
        let payload = base64::engine::general_purpose::STANDARD.encode("image");
        for value in [
            format!("data:image/png;base64,{payload}"),
            format!("image/png;base64,{payload}"),
        ] {
            let (media_type, got) = decode_data_url(&value).unwrap();
            assert_eq!(media_type, "image/png");
            assert_eq!(got, b"image");
        }
    }

    #[test]
    fn normalize_raster_image_to_square_png() {
        let mut source = RgbaImage::new(8, 4);
        for y in 0..4 {
            for x in 0..8 {
                source.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        let got = normalize(&encode_test_png(&source), "image/png", 32).unwrap();
        let decoded = image::load_from_memory(&got).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (32, 32));
        assert_eq!(decoded.get_pixel(0, 0)[3], 0);
    }

    #[test]
    fn normalize_svg() {
        let data = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="red"/></svg>"#;
        let got = normalize(data.as_slice(), "image/svg+xml", 32).unwrap();
        let decoded = image::load_from_memory(&got).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (32, 32));
    }

    #[test]
    fn dark_mode_variant_adds_surface_to_dark_transparent_icon() {
        let data = transparent_icon_png(Rgba([0, 0, 0, 255]));
        let (variant, analysis) = dark_mode_variant_with_analysis(&data, BackgroundMode::Auto);
        assert!(analysis.classified_dark);
        assert!(analysis.has_transparency);
        assert!(analysis.background_added);
        let variant = variant.unwrap();
        let decoded = image::load_from_memory(&variant).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0)[3], 0);
        assert!(decoded.get_pixel(16, 2)[3] > 0);
    }

    #[test]
    fn dark_mode_variant_leaves_opaque_dark_icon_unchanged_in_auto() {
        let data = opaque_icon_png(Rgba([8, 8, 8, 255]));
        let (variant, analysis) = dark_mode_variant_with_analysis(&data, BackgroundMode::Auto);
        assert!(analysis.classified_dark);
        assert!(!analysis.has_transparency);
        assert!(variant.is_none());

        let (variant, analysis) = dark_mode_variant_with_analysis(&data, BackgroundMode::Always);
        assert!(variant.is_some());
        assert!(analysis.background_added);
    }

    #[test]
    fn dark_mode_variant_ignores_small_transparent_fringe() {
        let mut source = RgbaImage::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                source.put_pixel(x, y, Rgba([8, 8, 8, 255]));
            }
        }
        for y in 0..2 {
            for x in 0..32 {
                source.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
        let data = encode_test_png(&source);
        let (variant, analysis) = dark_mode_variant_with_analysis(&data, BackgroundMode::Auto);
        assert!(analysis.classified_dark);
        assert!(!analysis.has_transparency);
        assert!(variant.is_none());
        assert!(analysis.transparent_ratio >= 0.06 && analysis.transparent_ratio <= 0.07);
    }

    #[test]
    fn dark_mode_variant_leaves_bright_icon_unchanged() {
        let data = transparent_icon_png(Rgba([255, 255, 255, 255]));
        let (variant, analysis) = dark_mode_variant_with_analysis(&data, BackgroundMode::Auto);
        assert!(!analysis.classified_dark);
        assert!(variant.is_none());

        let (variant, analysis) = dark_mode_variant_with_analysis(&data, BackgroundMode::Always);
        assert!(variant.is_some());
        assert!(analysis.background_added);
    }

    #[test]
    fn dark_mode_variant_never_mode() {
        let data = transparent_icon_png(Rgba([0, 0, 0, 255]));
        let (variant, analysis) = dark_mode_variant_with_analysis(&data, BackgroundMode::Never);
        assert!(analysis.classified_dark);
        assert!(variant.is_none());
    }

    #[test]
    fn icon_service_caches_and_deduplicates_remote_loads() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        use crate::remote::{
            CategoryDto, EntriesFilter, EntryDto, FeedCountersDto, FeedDto, RemoteError,
        };

        #[derive(Default)]
        struct State {
            calls: AtomicUsize,
        }

        struct FakeRemote(Arc<State>);

        impl RemoteInbox for FakeRemote {
            fn fetch_complete_selection(
                &self,
                _filter: &EntriesFilter,
            ) -> Result<(Vec<EntryDto>, i64), RemoteError> {
                Ok((Vec::new(), 0))
            }

            fn categories(&self) -> Result<Vec<CategoryDto>, RemoteError> {
                Ok(Vec::new())
            }

            fn feeds(&self) -> Result<Vec<FeedDto>, RemoteError> {
                Ok(Vec::new())
            }

            fn unread_counters(&self) -> Result<FeedCountersDto, RemoteError> {
                Ok(FeedCountersDto {
                    unreads: HashMap::new(),
                })
            }

            fn starred_total(&self) -> Result<i64, RemoteError> {
                Ok(0)
            }

            fn icon_data_url(&self, _feed_id: i64) -> Result<Option<String>, RemoteError> {
                self.0.calls.fetch_add(1, Ordering::SeqCst);
                let png = transparent_icon_png(Rgba([0, 0, 0, 255]));
                let encoded = base64::engine::general_purpose::STANDARD.encode(&png);
                Ok(Some(format!("data:image/png;base64,{encoded}")))
            }

            fn set_read_batch(&self, _entry_ids: &[i64], _read: bool) -> Result<(), RemoteError> {
                Ok(())
            }

            fn entry_starred(&self, _entry_id: i64) -> Result<bool, RemoteError> {
                Ok(false)
            }

            fn toggle_starred(&self, _entry_id: i64) -> Result<(), RemoteError> {
                Ok(())
            }
        }

        let state = Arc::new(State::default());
        let remote = FakeRemote(Arc::clone(&state));
        let service = IconService::new();

        let first = service.feed_icon(42, &remote);
        assert!(!first.regular.is_empty());

        let second = service.feed_icon(42, &remote);
        assert_eq!(first.regular, second.regular);

        assert_eq!(state.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn icon_service_returns_empty_for_missing_remote_icon() {
        use crate::remote::{
            CategoryDto, EntriesFilter, EntryDto, FeedCountersDto, FeedDto, RemoteError,
        };

        #[derive(Default)]
        struct FakeRemote;

        impl RemoteInbox for FakeRemote {
            fn fetch_complete_selection(
                &self,
                _filter: &EntriesFilter,
            ) -> Result<(Vec<EntryDto>, i64), RemoteError> {
                Ok((Vec::new(), 0))
            }

            fn categories(&self) -> Result<Vec<CategoryDto>, RemoteError> {
                Ok(Vec::new())
            }

            fn feeds(&self) -> Result<Vec<FeedDto>, RemoteError> {
                Ok(Vec::new())
            }

            fn unread_counters(&self) -> Result<FeedCountersDto, RemoteError> {
                Ok(FeedCountersDto {
                    unreads: HashMap::new(),
                })
            }

            fn starred_total(&self) -> Result<i64, RemoteError> {
                Ok(0)
            }

            fn icon_data_url(&self, _feed_id: i64) -> Result<Option<String>, RemoteError> {
                Ok(None)
            }

            fn set_read_batch(&self, _entry_ids: &[i64], _read: bool) -> Result<(), RemoteError> {
                Ok(())
            }

            fn entry_starred(&self, _entry_id: i64) -> Result<bool, RemoteError> {
                Ok(false)
            }

            fn toggle_starred(&self, _entry_id: i64) -> Result<(), RemoteError> {
                Ok(())
            }
        }

        let service = IconService::new();
        let icon = service.feed_icon(1, &FakeRemote);
        assert!(icon.regular.is_empty());
        assert!(icon.dark.is_empty());
    }

    #[test]
    fn icon_service_retries_failed_missing_and_malformed_loads() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct RetryRemote {
            calls: [AtomicUsize; 3],
            valid: String,
        }

        impl RemoteInbox for RetryRemote {
            remote_stubs!();

            fn icon_data_url(
                &self,
                feed_id: i64,
            ) -> Result<Option<String>, crate::remote::RemoteError> {
                let index = (feed_id - 1) as usize;
                let call = self.calls[index].fetch_add(1, Ordering::SeqCst);
                if call > 0 {
                    return Ok(Some(self.valid.clone()));
                }
                match feed_id {
                    1 => Err(crate::remote::RemoteError::Transport("offline".into())),
                    2 => Ok(None),
                    3 => Ok(Some("not an icon".into())),
                    _ => unreachable!(),
                }
            }
        }

        let png = transparent_icon_png(Rgba([255, 255, 255, 255]));
        let remote = RetryRemote {
            calls: std::array::from_fn(|_| AtomicUsize::new(0)),
            valid: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(png)
            ),
        };
        let service = IconService::new();

        for feed_id in 1..=3 {
            assert!(service.feed_icon(feed_id, &remote).regular.is_empty());
            assert!(!service.feed_icon(feed_id, &remote).regular.is_empty());
            assert!(!service.feed_icon(feed_id, &remote).regular.is_empty());
            assert_eq!(
                remote.calls[(feed_id - 1) as usize].load(Ordering::SeqCst),
                2
            );
        }
    }

    #[test]
    fn icon_service_cleans_up_panicked_load_and_wakes_waiter() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::mpsc;
        use std::time::{Duration, Instant};

        struct PanicRemote {
            calls: AtomicUsize,
            entered: (Mutex<bool>, Condvar),
            release: (Mutex<bool>, Condvar),
            valid: String,
        }

        impl RemoteInbox for PanicRemote {
            remote_stubs!();

            fn icon_data_url(
                &self,
                _feed_id: i64,
            ) -> Result<Option<String>, crate::remote::RemoteError> {
                if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    let mut entered = self.entered.0.lock().unwrap();
                    *entered = true;
                    self.entered.1.notify_all();
                    drop(entered);

                    let mut release = self.release.0.lock().unwrap();
                    while !*release {
                        release = self.release.1.wait(release).unwrap();
                    }
                    panic!("scripted icon load panic");
                }
                Ok(Some(self.valid.clone()))
            }
        }

        let png = transparent_icon_png(Rgba([255, 255, 255, 255]));
        let remote = Arc::new(PanicRemote {
            calls: AtomicUsize::new(0),
            entered: (Mutex::new(false), Condvar::new()),
            release: (Mutex::new(false), Condvar::new()),
            valid: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(png)
            ),
        });
        let service = Arc::new(IconService::new());
        let (leader_tx, leader_rx) = mpsc::channel();
        let leader_service = Arc::clone(&service);
        let leader_remote = Arc::clone(&remote);
        std::thread::spawn(move || {
            let panicked = catch_unwind(AssertUnwindSafe(|| {
                leader_service.feed_icon(42, leader_remote.as_ref())
            }))
            .is_err();
            leader_tx.send(panicked).unwrap();
        });

        let mut entered = remote.entered.0.lock().unwrap();
        while !*entered {
            entered = remote.entered.1.wait(entered).unwrap();
        }
        drop(entered);
        let slot = service.loads.lock().unwrap().get(&42).unwrap().clone();

        let (waiter_tx, waiter_rx) = mpsc::channel();
        let waiter_service = Arc::clone(&service);
        let waiter_remote = Arc::clone(&remote);
        std::thread::spawn(move || {
            waiter_tx
                .send(waiter_service.feed_icon(42, waiter_remote.as_ref()))
                .unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        while slot.waiters.load(Ordering::SeqCst) == 0 {
            assert!(
                Instant::now() < deadline,
                "waiter did not join single-flight load"
            );
            std::thread::yield_now();
        }

        *remote.release.0.lock().unwrap() = true;
        remote.release.1.notify_all();
        assert!(leader_rx.recv_timeout(Duration::from_secs(2)).unwrap());
        let icon = waiter_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(icon.regular.is_empty());
        assert_eq!(remote.calls.load(Ordering::SeqCst), 1);

        assert!(!service.feed_icon(42, remote.as_ref()).regular.is_empty());
        assert_eq!(remote.calls.load(Ordering::SeqCst), 2);
    }

    fn dark_mode_variant_with_analysis(
        data: &[u8],
        mode: BackgroundMode,
    ) -> (Option<Vec<u8>>, AppearanceAnalysis) {
        let variant = dark_mode_variant(data, mode).unwrap();
        let mut analysis = analyze_appearance(&image::load_from_memory(data).unwrap().to_rgba8());
        analysis.background_mode = mode;
        analysis.background_added = variant.is_some();
        (variant, analysis)
    }
}
