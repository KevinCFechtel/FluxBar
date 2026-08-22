//! Test-only icon compatibility helper invoked by `Build/test-icon-compat.sh`.

use std::io::Cursor;

use base64::Engine;
use fluxcore::icons::{BackgroundMode, DEFAULT_SIZE, dark_mode_variant, normalize_data_url};
use fluxcore::transport::Response;
use fluxcore::transport::response::Icon;
use image::codecs::png::PngEncoder;
use image::{ImageEncoder, Rgba, RgbaImage};

#[derive(serde::Deserialize)]
struct Fixture {
    cases: Vec<CaseInput>,
}

#[derive(serde::Deserialize)]
struct CaseInput {
    name: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    background: [u8; 4],
    #[serde(default)]
    foreground: [u8; 4],
    #[serde(default)]
    inset: u32,
    #[serde(default)]
    malformed: bool,
}

#[derive(serde::Serialize)]
struct DecodedImage {
    width: u32,
    height: u32,
    rgba: String,
    png: String,
}

#[derive(serde::Serialize)]
struct CaseOutput {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    regular: Option<DecodedImage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dark: Option<DecodedImage>,
    response: serde_json::Value,
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: icon-compat <fixture.json>");
    let input = std::fs::read_to_string(path).expect("read fixture");
    let fixture: Fixture = serde_json::from_str(&input).expect("parse fixture");

    let outputs: Vec<CaseOutput> = fixture
        .cases
        .into_iter()
        .map(|case| {
            let data_url = if case.malformed {
                "malformed".to_string()
            } else {
                synthetic_data_url(&case)
            };
            let regular = normalize_data_url(&data_url, DEFAULT_SIZE).unwrap_or_default();
            let dark = if regular.is_empty() {
                Vec::new()
            } else {
                dark_mode_variant(&regular, BackgroundMode::Auto)
                    .unwrap_or_default()
                    .unwrap_or_default()
            };
            let response = Response {
                ok: true,
                icon: Some(Icon {
                    regular: regular.clone(),
                    dark: dark.clone(),
                }),
                ..Response::default()
            };
            CaseOutput {
                name: case.name,
                regular: decode_image(&regular),
                dark: decode_image(&dark),
                response: serde_json::from_str(&response.to_json()).expect("parse response"),
            }
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string(&outputs).expect("encode output")
    );
}

fn synthetic_data_url(case: &CaseInput) -> String {
    let mut source = RgbaImage::new(case.width, case.height);
    for y in 0..case.height {
        for x in 0..case.width {
            let pixel = if x >= case.inset
                && x < case.width - case.inset
                && y >= case.inset
                && y < case.height - case.inset
            {
                case.foreground
            } else {
                case.background
            };
            source.put_pixel(x, y, Rgba(pixel));
        }
    }
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(
            source.as_raw(),
            source.width(),
            source.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("encode source image");
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(encoded)
    )
}

fn decode_image(data: &[u8]) -> Option<DecodedImage> {
    if data.is_empty() {
        return None;
    }
    let decoded = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .expect("detect output image")
        .decode()
        .expect("decode output image")
        .to_rgba8();
    Some(DecodedImage {
        width: decoded.width(),
        height: decoded.height(),
        rgba: base64::engine::general_purpose::STANDARD.encode(decoded.as_raw()),
        png: base64::engine::general_purpose::STANDARD.encode(data),
    })
}
