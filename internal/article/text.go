package article

import (
	"net/url"
	"strconv"
	"strings"
	"unicode/utf8"

	"golang.org/x/net/html"
)

const PreviewLimit = 600

type Preview struct {
	Text     string
	ImageURL string
}

var blockElements = map[string]bool{
	"address": true, "article": true, "aside": true, "blockquote": true,
	"div": true, "figcaption": true, "figure": true, "footer": true,
	"h1": true, "h2": true, "h3": true, "h4": true, "h5": true, "h6": true,
	"header": true, "li": true, "main": true, "nav": true, "p": true,
	"pre": true, "section": true, "table": true, "td": true, "th": true, "tr": true,
}

var ignoredElements = map[string]bool{
	"head": true, "noscript": true, "script": true, "style": true, "template": true,
}

// PlainText converts article HTML into a compact text preview. Block elements
// remain separated by line breaks, while markup and non-visible content are
// discarded. Image alt text is retained when the feed provides it.
func PlainText(value string, limit int) string {
	return Extract(value, "", limit).Text
}

// Extract returns a text preview and the first usable article image. Relative
// image references are resolved against the article URL.
func Extract(value, baseURL string, limit int) Preview {
	if strings.TrimSpace(value) == "" {
		return Preview{}
	}
	document, err := html.Parse(strings.NewReader(value))
	if err != nil {
		return Preview{Text: truncate(strings.Join(strings.Fields(value), " "), limit)}
	}

	var output strings.Builder
	appendNodeText(&output, document)
	lines := strings.Split(output.String(), "\n")
	cleaned := make([]string, 0, len(lines))
	for _, line := range lines {
		line = strings.Join(strings.Fields(line), " ")
		if line != "" {
			cleaned = append(cleaned, line)
		}
	}
	return Preview{
		Text:     truncate(strings.Join(cleaned, "\n"), limit),
		ImageURL: firstImageURL(document, baseURL),
	}
}

func firstImageURL(document *html.Node, baseURL string) string {
	var base *url.URL
	if parsed, err := url.Parse(baseURL); err == nil {
		base = parsed
	}
	var visit func(*html.Node) string
	visit = func(node *html.Node) string {
		if node.Type == html.ElementNode && (node.Data == "img" || node.Data == "source") && !tinyImage(node) {
			for _, name := range []string{"data-src", "data-original", "src", "data-srcset", "srcset"} {
				candidate := attribute(node, name)
				if strings.HasSuffix(name, "srcset") {
					candidate = srcsetURL(candidate)
				}
				if resolved := resolveImageURL(candidate, base); resolved != "" {
					return resolved
				}
			}
		}
		for child := node.FirstChild; child != nil; child = child.NextSibling {
			if candidate := visit(child); candidate != "" {
				return candidate
			}
		}
		return ""
	}
	return visit(document)
}

func resolveImageURL(candidate string, base *url.URL) string {
	candidate = strings.TrimSpace(candidate)
	if candidate == "" || strings.HasPrefix(strings.ToLower(candidate), "data:") {
		return ""
	}
	parsed, err := url.Parse(candidate)
	if err != nil {
		return ""
	}
	if !parsed.IsAbs() && base != nil {
		parsed = base.ResolveReference(parsed)
	}
	if parsed.Scheme != "http" && parsed.Scheme != "https" {
		return ""
	}
	return parsed.String()
}

func srcsetURL(value string) string {
	parts := strings.Split(value, ",")
	for index := len(parts) - 1; index >= 0; index-- {
		fields := strings.Fields(parts[index])
		if len(fields) > 0 {
			return fields[0]
		}
	}
	return ""
}

func tinyImage(node *html.Node) bool {
	width, widthOK := positiveDimension(attribute(node, "width"))
	height, heightOK := positiveDimension(attribute(node, "height"))
	return widthOK && heightOK && width <= 2 && height <= 2
}

func positiveDimension(value string) (int, bool) {
	value = strings.TrimSuffix(strings.TrimSpace(value), "px")
	dimension, err := strconv.Atoi(value)
	return dimension, err == nil && dimension >= 0
}

func appendNodeText(output *strings.Builder, node *html.Node) {
	if node.Type == html.ElementNode && ignoredElements[node.Data] {
		return
	}
	if node.Type == html.TextNode {
		output.WriteString(node.Data)
		return
	}
	if node.Type == html.ElementNode {
		switch node.Data {
		case "br":
			output.WriteByte('\n')
		case "img":
			if alt := attribute(node, "alt"); alt != "" {
				output.WriteString(" ")
				output.WriteString(alt)
				output.WriteString(" ")
			}
		}
		if blockElements[node.Data] {
			output.WriteByte('\n')
		}
	}
	for child := node.FirstChild; child != nil; child = child.NextSibling {
		appendNodeText(output, child)
	}
	if node.Type == html.ElementNode && blockElements[node.Data] {
		output.WriteByte('\n')
	}
}

func attribute(node *html.Node, name string) string {
	for _, attribute := range node.Attr {
		if attribute.Key == name {
			return strings.TrimSpace(attribute.Val)
		}
	}
	return ""
}

func truncate(value string, limit int) string {
	if limit <= 0 || utf8.RuneCountInString(value) <= limit {
		return value
	}
	runes := []rune(value)
	return strings.TrimSpace(string(runes[:limit-1])) + "…"
}
