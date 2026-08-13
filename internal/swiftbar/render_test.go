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
		"1 | image=dGl0bGU=",
		"**Feed**: Title | image=cG5n width=16 height=16",
		`bash="/tmp/plugin.sh"`,
		"param1=3",
	} {
		if !strings.Contains(output.String(), want) {
			t.Fatalf("output %q does not contain %q", output.String(), want)
		}
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
