//go:build compat
// +build compat

// article-compat is a test-only helper invoked by Build/test-article-compat.sh.
// It reads a JSON fixture of article inputs and prints one JSON object per case
// containing the preview text and image URL that the Go core would produce.
package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/KevinCFechtel/FluxBar/internal/miniflux"
	"github.com/KevinCFechtel/FluxBar/internal/model"
	minifluxClient "miniflux.app/v2/client"
)

type fixture struct {
	Cases []caseInput `json:"cases"`
}

type caseInput struct {
	Name       string            `json:"name"`
	Content    string            `json:"content"`
	BaseURL    string            `json:"base_url"`
	Limit      int               `json:"limit"`
	Enclosures []enclosureInput  `json:"enclosures"`
}

type enclosureInput struct {
	URL      string `json:"url"`
	MimeType string `json:"mime_type"`
}

type caseOutput struct {
	Name     string `json:"name"`
	Text     string `json:"text"`
	ImageURL string `json:"image_url"`
}

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintln(os.Stderr, "usage: article-compat <fixture.json>")
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
		entry := mapEntry(c)
		outputs = append(outputs, caseOutput{
			Name:     c.Name,
			Text:     entry.Preview,
			ImageURL: entry.ImageURL,
		})
	}

	encoded, err := json.Marshal(outputs)
	if err != nil {
		fmt.Fprintln(os.Stderr, "encode output:", err)
		os.Exit(2)
	}
	fmt.Println(string(encoded))
}

func mapEntry(c caseInput) model.Entry {
	enclosures := make(minifluxClient.Enclosures, 0, len(c.Enclosures))
	for _, e := range c.Enclosures {
		enclosures = append(enclosures, &minifluxClient.Enclosure{
			URL:      e.URL,
			MimeType: e.MimeType,
		})
	}
	return miniflux.MapEntryForCompat(&minifluxClient.Entry{
		ID:         1,
		URL:        c.BaseURL,
		Content:    c.Content,
		Enclosures: enclosures,
	})
}
