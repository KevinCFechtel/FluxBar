//! Test-only localization compatibility helper invoked by
//! `Build/test-localization-compat.sh`.

use std::path::PathBuf;

use fluxcore::localization::Localizer;

#[derive(Debug, serde::Deserialize)]
struct Fixture {
    cases: Vec<CaseInput>,
}

#[derive(Debug, serde::Deserialize)]
struct CaseInput {
    name: String,
    operation: String,
    #[serde(default)]
    locales: Vec<String>,
    key: String,
    #[serde(default)]
    fallback: String,
    #[serde(default, rename = "one_fallback")]
    one_fallback: String,
    #[serde(default, rename = "other_fallback")]
    other_fallback: String,
    #[serde(default)]
    count: i32,
}

#[derive(Debug, serde::Serialize)]
struct CaseOutput {
    name: String,
    text: String,
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: localization-compat <fixture.json>");
    let data = std::fs::read_to_string(PathBuf::from(&path)).expect("read fixture");
    let fixture: Fixture = serde_json::from_str(&data).expect("parse fixture");

    let outputs: Vec<CaseOutput> = fixture
        .cases
        .into_iter()
        .map(|case| {
            let localizer = Localizer::new(&case.locales);
            let text = match case.operation.as_str() {
                "text" => localizer.text(&case.key, &case.fallback),
                "plural" => localizer.plural(
                    &case.key,
                    &case.one_fallback,
                    &case.other_fallback,
                    case.count,
                ),
                other => panic!("unknown operation: {other}"),
            };
            CaseOutput {
                name: case.name,
                text,
            }
        })
        .collect();

    println!(
        "{}",
        serde_json::to_string(&outputs).expect("serialize output")
    );
}
