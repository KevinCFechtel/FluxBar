package swiftbar

import (
	"bytes"
	"strings"
	"testing"

	"github.com/KevinCFechtel/FluxBar/internal/model"
)

func TestRenderSwiftBarIncludesFeedIcon(t *testing.T) {
	var output bytes.Buffer
	err := Render(&output, []model.Entry{{
		ID: 3, FeedName: "Feed", Title: "Title", URL: "https://example.com", Icon: []byte("png"),
	}}, 1, Options{ShellPath: "/tmp/plugin.sh", SwiftBar: true, TitleIcon: []byte("title")})
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{
		"1 | templateImage=dGl0bGU= width=16 height=16",
		"**Feed**: Title | image=cG5n width=16 height=16",
		`bash="/tmp/plugin.sh"`,
		"param1=3",
	} {
		if !strings.Contains(output.String(), want) {
			t.Fatalf("output %q does not contain %q", output.String(), want)
		}
	}
}

func TestRenderUsesTemplateOnlyForTitleIcon(t *testing.T) {
	var output bytes.Buffer
	err := Render(&output, []model.Entry{{Title: "Article", Icon: []byte("feed")}}, 1, Options{TitleIcon: []byte("title")})
	if err != nil {
		t.Fatal(err)
	}
	lines := strings.Split(strings.TrimSpace(output.String()), "\n")
	if !strings.Contains(lines[0], "templateImage=") || strings.Contains(lines[0], " image=") {
		t.Fatalf("title line does not use a template image: %q", lines[0])
	}
	if !strings.Contains(lines[2], " image=") || strings.Contains(lines[2], "templateImage=") {
		t.Fatalf("feed line does not use a regular image: %q", lines[2])
	}
}

func TestRenderSanitizesProtocolCharacters(t *testing.T) {
	var output bytes.Buffer
	if err := Render(&output, []model.Entry{{FeedName: "A|B", Title: "C\nD"}}, 1, Options{}); err != nil {
		t.Fatal(err)
	}
	if strings.Contains(output.String(), "A|B") || strings.Contains(output.String(), "C\nD") {
		t.Fatalf("unsafe output: %q", output.String())
	}
}
