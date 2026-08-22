//! Lightweight localization compatible with Go's go-i18n usage.
//!
//! FluxBar only needs English and German today. This module embeds the same
//! translation JSON files used by the Go core and implements the subset of
//! go-i18n semantics actually exercised by FluxBar:
//!
//! - BCP-47 locale preferences with English fallback;
//! - simple message lookup with caller-supplied fallback;
//! - plural message lookup with `{{.Count}}` template substitution.

use std::collections::HashMap;

use serde_json::Value;

const EN_JSON: &str = include_str!("../../go-core/internal/localization/translations/en.json");
const DE_JSON: &str = include_str!("../../go-core/internal/localization/translations/de.json");

/// Localization catalog for a single language.
#[derive(Debug, Clone)]
pub struct Localizer {
    messages: HashMap<String, Value>,
}

impl Localizer {
    /// Creates a localizer for the first supported preferred locale, falling
    /// back to English when none match. Empty preferences also fall back to
    /// English, mirroring Go's `localization.New`.
    pub fn new(preferred_locales: &[String]) -> Self {
        let catalog_json = match resolve_supported_language(preferred_locales) {
            "de" => DE_JSON,
            _ => EN_JSON,
        };
        Self::from_json(catalog_json)
    }

    fn from_json(source: &str) -> Self {
        let messages: HashMap<String, Value> = serde_json::from_str(source).unwrap_or_default();
        Self { messages }
    }

    /// Returns the localized message for `key`, or `fallback` if the key is
    /// missing or not a simple string.
    pub fn text(&self, key: &str, fallback: &str) -> String {
        match self.messages.get(key) {
            Some(Value::String(text)) => text.clone(),
            _ => fallback.to_string(),
        }
    }

    /// Returns the plural form for `key` using `count`. If the key is missing
    /// or lacks the required plural form, the matching caller fallback is used.
    /// Substitutes `{{.Count}}` (with optional whitespace) for the integer count.
    pub fn plural(
        &self,
        key: &str,
        one_fallback: &str,
        other_fallback: &str,
        count: i64,
    ) -> String {
        let singular = count.unsigned_abs() == 1;
        let fallback = if singular {
            one_fallback
        } else {
            other_fallback
        };
        let form = if singular { "one" } else { "other" };
        let template = self
            .messages
            .get(key)
            .and_then(|value| value.get(form))
            .and_then(|value| value.as_str())
            .unwrap_or(fallback);
        apply_count_template(template, count)
    }
}

fn resolve_supported_language(preferred: &[String]) -> &'static str {
    for locale in preferred {
        let primary = locale.split('-').next().unwrap_or("").to_ascii_lowercase();
        match primary.as_str() {
            "de" => return "de",
            "en" => return "en",
            _ => continue,
        }
    }
    "en"
}

fn apply_count_template(template: &str, count: i64) -> String {
    let count_str = count.to_string();
    let mut result = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        result.push_str(&rest[..start]);
        match rest[start..].find("}}") {
            Some(end_offset) => {
                let inner = &rest[start + 2..start + end_offset];
                if inner.trim() == ".Count" {
                    result.push_str(&count_str);
                } else {
                    result.push_str(&rest[start..start + end_offset + 2]);
                }
                rest = &rest[start + end_offset + 2..];
            }
            None => {
                result.push_str(&rest[start..]);
                break;
            }
        }
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representative_locales_and_fallbacks() {
        let cases = vec![
            ("en-US", "Refresh"),
            ("de-DE", "Aktualisieren"),
            ("fr-FR", "Refresh"),
        ];
        for (locale, expected) in cases {
            let localizer = Localizer::new(&[locale.to_string()]);
            assert_eq!(localizer.text("menu.refresh", "fallback"), expected);
        }
    }

    #[test]
    fn no_locale_preference_uses_english() {
        let localizer = Localizer::new(&[]);
        assert_eq!(localizer.text("menu.refresh", "fallback"), "Refresh");
    }

    #[test]
    fn unknown_key_uses_caller_fallback() {
        let localizer = Localizer::new(&["de-DE".to_string()]);
        assert_eq!(localizer.text("missing.key", "Fallback"), "Fallback");
    }

    #[test]
    fn pluralization_german() {
        let localizer = Localizer::new(&["de".to_string()]);
        assert_eq!(
            localizer.plural(
                "status.unread_count",
                "FluxBar — {{.Count}} unread article",
                "FluxBar — {{.Count}} unread articles",
                1
            ),
            "FluxBar — 1 ungelesener Artikel"
        );
        assert_eq!(
            localizer.plural(
                "status.unread_count",
                "FluxBar — {{.Count}} unread article",
                "FluxBar — {{.Count}} unread articles",
                2
            ),
            "FluxBar — 2 ungelesene Artikel"
        );
    }

    #[test]
    fn pluralization_english() {
        let localizer = Localizer::new(&["en".to_string()]);
        assert_eq!(
            localizer.plural(
                "status.unread_count",
                "FluxBar — {{.Count}} unread article",
                "FluxBar — {{.Count}} unread articles",
                1
            ),
            "FluxBar — 1 unread article"
        );
        assert_eq!(
            localizer.plural(
                "status.unread_count",
                "FluxBar — {{.Count}} unread article",
                "FluxBar — {{.Count}} unread articles",
                5
            ),
            "FluxBar — 5 unread articles"
        );
    }

    #[test]
    fn pluralization_fallback() {
        let localizer = Localizer::new(&["de".to_string()]);
        assert_eq!(
            localizer.plural("missing.plural", "{{.Count}} item", "{{.Count}} items", 2),
            "2 items"
        );
    }

    #[test]
    fn count_template_with_whitespace() {
        let localizer = Localizer::new(&["en".to_string()]);
        // Go's text/template accepts both {{.Count}} and {{ .Count }}.
        assert_eq!(
            localizer.plural(
                "missing.plural.with.spaces",
                "{{ .Count }} unread article",
                "{{ .Count }} unread articles",
                3
            ),
            "3 unread articles"
        );
    }
}
