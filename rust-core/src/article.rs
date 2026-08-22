//! Go-compatible article preview and image extraction.
//!
//! Ported from `go-core/internal/article/text.go`. The public behavior is
//! intentionally narrow: produce the same `preview` text and first usable
//! `image_url` that the Go core writes into SQLite during refresh.

use std::cell::RefCell;

use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};

pub const PREVIEW_LIMIT: usize = 600;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Preview {
    pub text: String,
    pub image_url: String,
}

const IGNORED_ELEMENTS: &[&str] = &["head", "noscript", "script", "style", "template"];
const BLOCK_ELEMENTS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "div",
    "figcaption",
    "figure",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "li",
    "main",
    "nav",
    "p",
    "pre",
    "section",
    "table",
    "td",
    "th",
    "tr",
];
const IMAGE_ATTRIBUTES: &[&str] = &["data-src", "data-original", "src", "data-srcset", "srcset"];

pub fn plain_text(value: &str, limit: usize) -> String {
    extract(value, "", limit).text
}

pub fn extract(value: &str, base_url: &str, limit: usize) -> Preview {
    if value.trim().is_empty() {
        return Preview::default();
    }
    let base = parse_base_url(base_url);
    let dom = match parse_html(value) {
        Some(dom) => dom,
        None => {
            return Preview {
                text: truncate(&collapse_whitespace(value), limit),
                image_url: String::new(),
            };
        }
    };
    let mut output = String::new();
    append_node_text(&mut output, &dom.document);
    let text = normalize_lines(&output, limit);
    let image_url = first_image_url(&dom.document, base.as_ref());
    Preview { text, image_url }
}

pub fn resolve_image_url(candidate: &str, base_url: &str) -> String {
    let base = parse_base_url(base_url);
    resolve_image_url_with_base(candidate, base.as_ref())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnclosureInput {
    pub url: String,
    pub mime_type: String,
}

pub fn first_image_enclosure_url(enclosures: &[EnclosureInput], article_url: &str) -> String {
    let base = parse_base_url(article_url);
    for enclosure in enclosures {
        let media_type = enclosure
            .mime_type
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !media_type.starts_with("image/") {
            continue;
        }
        let resolved = resolve_image_url_with_base(&enclosure.url, base.as_ref());
        if !resolved.is_empty() {
            return resolved;
        }
    }
    String::new()
}

fn parse_base_url(base_url: &str) -> Option<url::Url> {
    url::Url::parse(base_url).ok()
}

fn parse_html(value: &str) -> Option<RcDom> {
    let result = std::panic::catch_unwind(|| {
        parse_document(RcDom::default(), Default::default())
            .from_utf8()
            .one(value.as_bytes())
    });
    result.ok()
}

fn append_node_text(output: &mut String, node: &Handle) {
    match &node.data {
        NodeData::Element { name, attrs, .. } => {
            let tag: &str = name.local.as_ref();
            if IGNORED_ELEMENTS.contains(&tag) {
                return;
            }
            if tag == "br" {
                output.push('\n');
            } else if tag == "img" {
                if let Some(alt) = attribute(attrs, "alt") {
                    output.push(' ');
                    output.push_str(&alt);
                    output.push(' ');
                }
            }
            if BLOCK_ELEMENTS.contains(&tag) {
                output.push('\n');
            }
            for child in node.children.borrow().iter() {
                append_node_text(output, child);
            }
            if BLOCK_ELEMENTS.contains(&tag) {
                output.push('\n');
            }
        }
        NodeData::Text { contents } => {
            output.push_str(&contents.borrow());
        }
        _ => {
            for child in node.children.borrow().iter() {
                append_node_text(output, child);
            }
        }
    }
}

fn normalize_lines(value: &str, limit: usize) -> String {
    let lines: Vec<String> = value
        .split('\n')
        .map(collapse_whitespace)
        .filter(|line| !line.is_empty())
        .collect();
    truncate(&lines.join("\n"), limit)
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(value: &str, limit: usize) -> String {
    if limit == 0 || value.chars().count() <= limit {
        return value.to_string();
    }
    let truncated: String = value.chars().take(limit - 1).collect();
    format!("{}…", truncated.trim_end())
}

fn first_image_url(node: &Handle, base: Option<&url::Url>) -> String {
    match &node.data {
        NodeData::Element { name, attrs, .. } => {
            let tag: &str = name.local.as_ref();
            if (tag == "img" || tag == "source") && !tiny_image(attrs) {
                for attr_name in IMAGE_ATTRIBUTES {
                    let candidate = attribute(attrs, attr_name).unwrap_or_default();
                    let candidate = if attr_name.ends_with("srcset") {
                        srcset_url(&candidate)
                    } else {
                        candidate
                    };
                    let resolved = resolve_image_url_with_base(&candidate, base);
                    if !resolved.is_empty() {
                        return resolved;
                    }
                }
            }
        }
        _ => {}
    }
    for child in node.children.borrow().iter() {
        let candidate = first_image_url(child, base);
        if !candidate.is_empty() {
            return candidate;
        }
    }
    String::new()
}

fn resolve_image_url_with_base(candidate: &str, base: Option<&url::Url>) -> String {
    let candidate = candidate.trim();
    if candidate.is_empty() || candidate.to_ascii_lowercase().starts_with("data:") {
        return String::new();
    }
    let parsed = match url::Url::parse(candidate) {
        Ok(url) => url,
        Err(_) => match base {
            Some(base) => match base.join(candidate) {
                Ok(url) => url,
                Err(_) => return String::new(),
            },
            None => return String::new(),
        },
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return String::new();
    }
    parsed.to_string()
}

fn srcset_url(value: &str) -> String {
    let parts: Vec<&str> = value.split(',').collect();
    for part in parts.iter().rev() {
        let fields: Vec<&str> = part.split_whitespace().collect();
        if !fields.is_empty() {
            return fields[0].to_string();
        }
    }
    String::new()
}

fn tiny_image(attrs: &RefCell<Vec<html5ever::Attribute>>) -> bool {
    let width = attribute(attrs, "width")
        .as_deref()
        .and_then(positive_dimension);
    let height = attribute(attrs, "height")
        .as_deref()
        .and_then(positive_dimension);
    matches!((width, height), (Some(w), Some(h)) if w <= 2 && h <= 2)
}

fn positive_dimension(value: &str) -> Option<i32> {
    let value = value.trim().trim_end_matches("px").trim();
    value.parse::<i32>().ok().filter(|&d| d >= 0)
}

fn attribute(attrs: &RefCell<Vec<html5ever::Attribute>>, name: &str) -> Option<String> {
    for attr in attrs.borrow().iter() {
        if attr.name.local.as_ref() == name {
            return Some(attr.value.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_extracts_readable_preview() {
        let html = r#"<html><head><title>Ignored</title><style>body{}</style></head><body>"#
            .to_string()
            + r#"<h1>Eine &amp; zwei</h1><p>Erster <strong>Absatz</strong><br>zweite Zeile.</p>"#
            + r#"<figure><img src="photo.jpg" alt="Bildbeschreibung"><figcaption>Bildtext</figcaption></figure>"#
            + r#"<script>alert("ignored")</script></body></html>"#;
        assert_eq!(
            plain_text(&html, PREVIEW_LIMIT),
            "Eine & zwei\nErster Absatz\nzweite Zeile.\nBildbeschreibung\nBildtext"
        );
    }

    #[test]
    fn plain_text_truncates_by_runes() {
        let html = format!("<p>{}</p>", "ä".repeat(20));
        assert_eq!(plain_text(&html, 10), format!("{}…", "ä".repeat(9)));
    }

    #[test]
    fn plain_text_handles_empty_content() {
        assert_eq!(plain_text(" \n ", PREVIEW_LIMIT), "");
    }

    #[test]
    fn extract_finds_article_image() {
        let preview = extract(
            r#"<p>Text</p><img width="1" height="1" src="https://tracker.example/pixel.gif"><img src="/images/article.jpg" alt="Article">"#,
            "https://news.example/posts/1",
            PREVIEW_LIMIT,
        );
        assert_eq!(preview.text, "Text\nArticle");
        assert_eq!(preview.image_url, "https://news.example/images/article.jpg");
    }

    #[test]
    fn extract_supports_lazy_and_responsive_images() {
        let preview = extract(
            r#"<picture><source srcset="small.jpg 320w, large.jpg 1280w"><img data-src="fallback.jpg"></picture>"#,
            "https://example.com/article/",
            PREVIEW_LIMIT,
        );
        assert_eq!(preview.image_url, "https://example.com/article/large.jpg");
    }

    #[test]
    fn extract_resolves_protocol_relative_url() {
        let preview = extract(
            r#"<img src="//cdn.example.com/image.jpg">"#,
            "https://example.com/",
            PREVIEW_LIMIT,
        );
        assert_eq!(preview.image_url, "https://cdn.example.com/image.jpg");
    }

    #[test]
    fn extract_returns_empty_for_whitespace_only() {
        let preview = extract("   ", "https://example.com/", PREVIEW_LIMIT);
        assert!(preview.text.is_empty());
        assert!(preview.image_url.is_empty());
    }

    #[test]
    fn image_url_rejects_data_and_non_http() {
        assert_eq!(
            resolve_image_url("data:image/png;base64,abc", "https://example.com/"),
            ""
        );
        assert_eq!(
            resolve_image_url("javascript:void(0)", "https://example.com/"),
            ""
        );
        assert_eq!(
            resolve_image_url("file:///tmp/x.jpg", "https://example.com/"),
            ""
        );
    }

    #[test]
    fn enclosure_fallback_uses_first_image() {
        let enclosures = vec![
            EnclosureInput {
                url: "https://example.com/audio.mp3".to_string(),
                mime_type: "audio/mpeg".to_string(),
            },
            EnclosureInput {
                url: "/images/cover.jpg".to_string(),
                mime_type: " Image/JPEG; charset=binary ".to_string(),
            },
        ];
        assert_eq!(
            first_image_enclosure_url(&enclosures, "https://example.com/article/"),
            "https://example.com/images/cover.jpg"
        );
    }

    #[test]
    fn enclosure_fallback_ignores_non_image_and_invalid() {
        let enclosures = vec![
            EnclosureInput {
                url: "https://example.com/video.mp4".to_string(),
                mime_type: "video/mp4".to_string(),
            },
            EnclosureInput {
                url: "file:///tmp/image.jpg".to_string(),
                mime_type: "image/jpeg".to_string(),
            },
        ];
        assert_eq!(
            first_image_enclosure_url(&enclosures, "https://example.com/"),
            ""
        );
    }
}
