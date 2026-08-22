package inbox

import (
	"crypto/sha256"
	"encoding/hex"
	"testing"
)

// Characterization test locking the account ID derivation for the Rust
// compatibility migration. The expected value is computed independently here
// so the production function cannot silently drift.
func TestAccountIDDerivation(t *testing.T) {
	got := AccountID("https://miniflux.example", "secret")
	sum := sha256.Sum256([]byte("https://miniflux.example\x00secret"))
	expected := hex.EncodeToString(sum[:])
	if got != expected {
		t.Fatalf("AccountID = %q, want %q", got, expected)
	}
	if len(got) != 64 {
		t.Fatalf("AccountID length = %d, want 64 hex characters", len(got))
	}
}
