//! Test-only article-processing compatibility helper invoked by
//! `Build/test-article-compat.sh`.

use std::path::PathBuf;

use fluxcore::article::{EnclosureInput, PREVIEW_LIMIT, extract, first_image_enclosure_url};

#[derive(Debug, serde::Deserialize)]
struct Fixture {
    cases: Vec<CaseInput>,
}

#[derive(Debug, serde::Deserialize)]
struct CaseInput {
    name: String,
    content: String,
    #[serde(rename = "base_url")]
    base_url: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    enclosures: Vec<EnclosureFixture>,
}

#[derive(Debug, serde::Deserialize)]
struct EnclosureFixture {
    url: String,
    #[serde(rename = "mime_type")]
    mime_type: String,
}

#[derive(Debug, serde::Serialize)]
struct CaseOutput {
    name: String,
    text: String,
    #[serde(rename = "image_url")]
    image_url: String,
}

fn default_limit() -> usize {
    PREVIEW_LIMIT
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: article-compat <fixture.json>");
    let data = std::fs::read_to_string(PathBuf::from(&path)).expect("read fixture");
    let fixture: Fixture = serde_json::from_str(&data).expect("parse fixture");

    let outputs: Vec<CaseOutput> = fixture
        .cases
        .into_iter()
        .map(|case| {
            let preview = extract(&case.content, &case.base_url, case.limit);
            let image_url = if preview.image_url.is_empty() {
                let enclosures: Vec<EnclosureInput> = case
                    .enclosures
                    .into_iter()
                    .map(|enclosure| EnclosureInput {
                        url: enclosure.url,
                        mime_type: enclosure.mime_type,
                    })
                    .collect();
                first_image_enclosure_url(&enclosures, &case.base_url)
            } else {
                preview.image_url
            };
            CaseOutput {
                name: case.name,
                text: preview.text,
                image_url,
            }
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string(&outputs).expect("serialize output")
    );
}
