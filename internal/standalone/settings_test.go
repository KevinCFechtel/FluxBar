package standalone

import "testing"

func TestValidateSettings(t *testing.T) {
	got, err := validateSettings(Settings{
		Server: " https://miniflux.example.com/ ",
		APIKey: " secret ",
	})
	if err != nil {
		t.Fatal(err)
	}
	if got.Server != "https://miniflux.example.com" || got.APIKey != "secret" {
		t.Fatalf("validateSettings() = %#v", got)
	}
}

func TestValidateSettingsRejectsInvalidValues(t *testing.T) {
	tests := []Settings{
		{},
		{Server: "miniflux.example.com", APIKey: "secret"},
		{Server: "ftp://miniflux.example.com", APIKey: "secret"},
		{Server: "https://miniflux.example.com"},
	}
	for _, test := range tests {
		if _, err := validateSettings(test); err == nil {
			t.Fatalf("validateSettings(%#v) succeeded", test)
		}
	}
}
