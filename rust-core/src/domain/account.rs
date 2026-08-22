//! Account identity derivation.
//!
//! Ported from `go-core/internal/inbox/service.go` (`AccountID`):
//! SHA-256 over `server + "\x00" + apiKey`, rendered as lowercase hex.
//! The derivation must stay byte-identical because the value scopes every
//! SQLite row during the parallel Go/Rust period.

pub fn account_id(server: &str, api_key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(server.as_bytes());
    hasher.update([0u8]);
    hasher.update(api_key.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_go_derivation_vector() {
        // Cross-language vector: identical to TestAccountIDDerivation in
        // go-core/internal/inbox (server + NUL + apiKey, lowercase hex),
        // independently verified with shasum.
        assert_eq!(
            account_id("https://miniflux.example", "secret"),
            "81c8e5dd5e18edfda1b40509ab407f57b9c93f7e53d11c05504268799ded6d26"
        );
    }

    #[test]
    fn produces_64_lowercase_hex_characters() {
        let id = account_id("https://a.example", "k");
        assert_eq!(id.len(), 64);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn distinguishes_servers_and_keys() {
        assert_ne!(account_id("https://a", "k"), account_id("https://b", "k"));
        assert_ne!(account_id("https://a", "k"), account_id("https://a", "j"));
    }
}
