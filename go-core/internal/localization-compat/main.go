//go:build compat
// +build compat

// localization-compat is a test-only helper invoked by
// Build/test-localization-compat.sh. It reads a JSON fixture of localization
// inputs and prints one JSON object per case containing the string the Go core
// would produce.
package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/KevinCFechtel/FluxBar/internal/localization"
)

type fixture struct {
	Cases []caseInput `json:"cases"`
}

type caseInput struct {
	Name          string   `json:"name"`
	Operation     string   `json:"operation"`
	Locales       []string `json:"locales"`
	Key           string   `json:"key"`
	Fallback      string   `json:"fallback"`
	OneFallback   string   `json:"one_fallback"`
	OtherFallback string   `json:"other_fallback"`
	Count         int      `json:"count"`
}

type caseOutput struct {
	Name string `json:"name"`
	Text string `json:"text"`
}

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintln(os.Stderr, "usage: localization-compat <fixture.json>")
		os.Exit(2)
	}
	data, err := os.ReadFile(os.Args[1])
	if err != nil {
		fmt.Fprintln(os.Stderr, "read fixture:", err)
		os.Exit(2)
	}
	var fixture fixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		fmt.Fprintln(os.Stderr, "parse fixture:", err)
		os.Exit(2)
	}

	outputs := make([]caseOutput, 0, len(fixture.Cases))
	for _, c := range fixture.Cases {
		localizer, err := localization.New(c.Locales...)
		if err != nil {
			fmt.Fprintln(os.Stderr, "create localizer:", err)
			os.Exit(2)
		}
		var text string
		switch c.Operation {
		case "text":
			text = localizer.Text(c.Key, c.Fallback)
		case "plural":
			text = localizer.Plural(
				c.Key,
				c.OneFallback,
				c.OtherFallback,
				c.Count,
				map[string]any{"Count": c.Count},
			)
		default:
			fmt.Fprintln(os.Stderr, "unknown operation:", c.Operation)
			os.Exit(2)
		}
		outputs = append(outputs, caseOutput{Name: c.Name, Text: text})
	}

	encoded, err := json.Marshal(outputs)
	if err != nil {
		fmt.Fprintln(os.Stderr, "encode output:", err)
		os.Exit(2)
	}
	fmt.Println(string(encoded))
}
