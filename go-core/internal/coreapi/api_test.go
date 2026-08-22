package coreapi

import (
	"encoding/json"
	"testing"
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
