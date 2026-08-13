package config

import "testing"

func TestResolveUsesEnvironmentBeforeLinkedValues(t *testing.T) {
	t.Setenv(serverEnvironmentVariable, "https://runtime.example")
	t.Setenv(apiKeyEnvironmentVariable, "runtime-key")

	got, err := Resolve("https://linked.example", "linked-key")
	if err != nil {
		t.Fatal(err)
	}
	if got.Server != "https://runtime.example" || got.APIKey != "runtime-key" {
		t.Fatalf("Resolve() = %#v", got)
	}
}

func TestResolveRejectsMissingValues(t *testing.T) {
	t.Setenv(serverEnvironmentVariable, "")
	t.Setenv(apiKeyEnvironmentVariable, "")
	if _, err := Resolve("", ""); err == nil {
		t.Fatal("Resolve() returned no error")
	}
}
