package coreapi

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"path/filepath"
	"testing"

	_ "github.com/mattn/go-sqlite3"
)

func TestLocalizeRequest(t *testing.T) {
	runtime := New(nil)
	responseJSON := runtime.HandleJSON(`{"operation":"localize","locales":["de-DE"],"key":"navigation.unread","fallback":"Unread"}`)
	var response Response
	if err := json.Unmarshal([]byte(responseJSON), &response); err != nil {
		t.Fatal(err)
	}
	if !response.OK || response.Text != "Ungelesen" {
		t.Fatalf("response = %#v", response)
	}
}

func TestInvalidAndUnconfiguredRequests(t *testing.T) {
	runtime := New(nil)
	for _, request := range []string{
		`not-json`,
		`{"operation":"refresh","selection":{"kind":"all"}}`,
		`{"operation":"unknown"}`,
	} {
		var response Response
		if err := json.Unmarshal([]byte(runtime.HandleJSON(request)), &response); err != nil {
			t.Fatalf("decode response for %q: %v", request, err)
		}
		if response.OK || response.Error == "" {
			t.Fatalf("response for %q = %#v", request, response)
		}
	}
}

func TestConfigureValidatesCredentials(t *testing.T) {
	runtime := New(nil)
	for _, request := range []string{
		`{"operation":"configure","server":"not-a-url","apiKey":"secret"}`,
		`{"operation":"configure","server":"https://miniflux.example","apiKey":""}`,
		`{"operation":"configure","server":"http://exa mple.com","apiKey":"secret"}`,
		`{"operation":"configure","server":"http://[::1","apiKey":"secret"}`,
		`{"operation":"configure","server":"http://example.com:bad","apiKey":"secret"}`,
	} {
		var response Response
		if err := json.Unmarshal([]byte(runtime.HandleJSON(request)), &response); err != nil {
			t.Fatal(err)
		}
		if response.OK || response.Error == "" {
			t.Fatalf("response = %#v", response)
		}
	}
}

func TestValidateConfigurationAcceptsOutOfRangeNumericPort(t *testing.T) {
	server, err := validateConfiguration("http://example.com:65536", "secret", nil)
	if err != nil || server != "http://example.com:65536" {
		t.Fatalf("validateConfiguration() = %q, %v", server, err)
	}
}

func TestConfigureSwitchesAccountsAndStaleGenerationStillUpserts(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	runtime := New(nil)

	configure := func(server, key string, generation int) {
		t.Helper()
		response := runtime.HandleJSON(fmt.Sprintf(
			`{"operation":"configure","server":%q,"apiKey":%q,"configurationGeneration":%d}`,
			server, key, generation,
		))
		var decoded Response
		if err := json.Unmarshal([]byte(response), &decoded); err != nil || !decoded.OK {
			t.Fatalf("configure response=%s error=%v", response, err)
		}
	}

	configure("https://a.example", "a", 1)
	engineA := runtime.currentEngine()
	configure("https://b.example", "b", 2)
	engineB := runtime.currentEngine()
	if engineA == engineB {
		t.Fatal("account B did not replace account A")
	}
	configure("https://stale.example", "stale", 1)
	if runtime.currentEngine() != engineB {
		t.Fatal("stale generation replaced the current engine")
	}
	configure("https://a.example", "a", 3)
	if runtime.currentEngine() == engineB {
		t.Fatal("newer account A generation did not replace account B")
	}

	database, err := sql.Open("sqlite3", filepath.Join(home, "Library", "Application Support", "FluxBar", "inbox.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()
	var accounts int
	if err := database.QueryRow(`SELECT COUNT(*) FROM accounts`).Scan(&accounts); err != nil {
		t.Fatal(err)
	}
	if accounts != 3 {
		t.Fatalf("account rows=%d, want 3 including stale configure", accounts)
	}
}
