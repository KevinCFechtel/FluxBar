package article

import (
	"strings"
	"testing"
)

func TestPlainTextExtractsReadablePreview(t *testing.T) {
	html := `<html><head><title>Ignored</title><style>body{}</style></head><body>` +
		`<h1>Eine &amp; zwei</h1><p>Erster <strong>Absatz</strong><br>zweite Zeile.</p>` +
		`<figure><img src="photo.jpg" alt="Bildbeschreibung"><figcaption>Bildtext</figcaption></figure>` +
		`<script>alert("ignored")</script></body></html>`
	want := "Eine & zwei\nErster Absatz\nzweite Zeile.\nBildbeschreibung\nBildtext"
	if got := PlainText(html, PreviewLimit); got != want {
		t.Fatalf("PlainText() = %q, want %q", got, want)
	}
}

func TestPlainTextTruncatesByRunes(t *testing.T) {
	got := PlainText("<p>"+strings.Repeat("ä", 20)+"</p>", 10)
	if got != strings.Repeat("ä", 9)+"…" {
		t.Fatalf("PlainText() = %q", got)
	}
}

func TestPlainTextHandlesEmptyContent(t *testing.T) {
	if got := PlainText(" \n ", PreviewLimit); got != "" {
		t.Fatalf("PlainText() = %q", got)
	}
}

func TestExtractFindsArticleImage(t *testing.T) {
	preview := Extract(
		`<p>Text</p><img width="1" height="1" src="https://tracker.example/pixel.gif"><img src="/images/article.jpg" alt="Article">`,
		"https://news.example/posts/1",
		PreviewLimit,
	)
	if preview.Text != "Text\nArticle" {
		t.Fatalf("text = %q", preview.Text)
	}
	if preview.ImageURL != "https://news.example/images/article.jpg" {
		t.Fatalf("image URL = %q", preview.ImageURL)
	}
}

func TestExtractSupportsLazyAndResponsiveImages(t *testing.T) {
	preview := Extract(`<picture><source srcset="small.jpg 320w, large.jpg 1280w"><img data-src="fallback.jpg"></picture>`, "https://example.com/article/", PreviewLimit)
	if preview.ImageURL != "https://example.com/article/large.jpg" {
		t.Fatalf("image URL = %q", preview.ImageURL)
	}
}
