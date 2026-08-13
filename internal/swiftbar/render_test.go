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
		ID: 3, FeedName: "Feed", Title: "Title", URL: "https://example.com", Preview: "First line\nSecond line", Icon: []byte("png"), DarkIcon: []byte("dark"),
	}}, 1, Options{ShellPath: "/tmp/plugin.sh", SwiftBar: true, DarkMode: true, TitleIcon: []byte("title")})
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{
		"1 | templateImage=dGl0bGU= width=16 height=16",
		"**Feed**: Title | image=ZGFyaw== width=16 height=16",
		`bash="/tmp/plugin.sh"`,
		"param1=3",
		`tooltip="First line\nSecond line"`,
	} {
		if !strings.Contains(output.String(), want) {
			t.Fatalf("output %q does not contain %q", output.String(), want)
		}
	}
}

func TestRenderOmitsTooltipForXbar(t *testing.T) {
	var output bytes.Buffer
	if err := Render(&output, []model.Entry{{Title: "Article", Preview: "Preview"}}, 1, Options{SwiftBar: false}); err != nil {
		t.Fatal(err)
	}
	if strings.Contains(output.String(), "tooltip=") {
		t.Fatalf("xbar output contains unsupported tooltip: %q", output.String())
	}
}

func TestRenderUsesTemplateOnlyForTitleIcon(t *testing.T) {
	var output bytes.Buffer
	err := Render(&output, []model.Entry{{Title: "Article", Icon: []byte("feed"), DarkIcon: []byte("dark")}}, 1, Options{TitleIcon: []byte("title")})
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
	if strings.Contains(lines[2], ",") {
		t.Fatalf("xbar output unexpectedly contains a dark-mode image: %q", lines[2])
	}
}

func TestRenderSelectsAppearanceBeforeWritingImage(t *testing.T) {
	entry := model.Entry{Title: "Article", Icon: []byte("light"), DarkIcon: []byte("dark")}
	for _, test := range []struct {
		name     string
		darkMode bool
		want     string
		notWant  string
	}{
		{name: "light", want: "bGlnaHQ=", notWant: "ZGFyaw=="},
		{name: "dark", darkMode: true, want: "ZGFyaw==", notWant: "bGlnaHQ="},
	} {
		t.Run(test.name, func(t *testing.T) {
			var output bytes.Buffer
			if err := Render(&output, []model.Entry{entry}, 1, Options{SwiftBar: true, DarkMode: test.darkMode}); err != nil {
				t.Fatal(err)
			}
			if !strings.Contains(output.String(), "image="+test.want) || strings.Contains(output.String(), test.notWant) {
				t.Fatalf("unexpected output: %q", output.String())
			}
			if strings.Contains(output.String(), "image="+test.want+",") {
				t.Fatalf("output contains a SwiftBar image pair: %q", output.String())
			}
		})
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
