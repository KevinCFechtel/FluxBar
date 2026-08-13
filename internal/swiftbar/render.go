package swiftbar

import (
	"encoding/base64"
	"fmt"
	"io"
	"strconv"
	"strings"

	"github.com/KevinCFechtel/FluxBar/internal/model"
)

type Options struct {
	ShellPath string
	SwiftBar  bool
	DarkMode  bool
	TitleIcon []byte
}

// Render writes the textual xbar/SwiftBar menu protocol.
func Render(writer io.Writer, entries []model.Entry, total int, options Options) error {
	title := strconv.Itoa(total)
	if len(options.TitleIcon) > 0 {
		// The menu bar logo is monochrome. templateImage lets macOS tint it for
		// both light and dark menu bars; image= would keep its white source pixels.
		title += " | templateImage=" + base64.StdEncoding.EncodeToString(options.TitleIcon)
		title += " width=16 height=16"
	}
	if _, err := fmt.Fprintln(writer, title); err != nil {
		return err
	}
	if _, err := fmt.Fprintln(writer, "---"); err != nil {
		return err
	}

	for _, entry := range entries {
		label := entryLabel(entry, options.SwiftBar)
		parameters := imageParameters(iconForAppearance(entry, options.DarkMode), options.SwiftBar)
		if options.ShellPath != "" {
			parameters += " bash=" + quote(options.ShellPath)
			parameters += " refresh=true param1=" + strconv.FormatInt(entry.ID, 10) + " terminal=false"
		}
		if options.SwiftBar {
			parameters += " md=true"
			if entry.Preview != "" {
				parameters += " tooltip=" + quote(entry.Preview)
			}
		} else {
			parameters += " ansi=true"
		}
		if entry.URL != "" {
			parameters += " href=" + quote(entry.URL)
		}
		if _, err := fmt.Fprintf(writer, "%s |%s\n", label, parameters); err != nil {
			return err
		}
	}
	return nil
}

func entryLabel(entry model.Entry, swiftBar bool) string {
	feed := clean(entry.FeedName)
	title := clean(entry.Title)
	if feed == "" {
		return title
	}
	if swiftBar {
		return "**" + escapeMarkdown(feed) + "**: " + title
	}
	return "\x1b[37m" + feed + ": \x1b[0m" + title
}

func iconForAppearance(entry model.Entry, darkMode bool) []byte {
	if darkMode && len(entry.DarkIcon) > 0 {
		return entry.DarkIcon
	}
	return entry.Icon
}

func imageParameters(icon []byte, swiftBar bool) string {
	if len(icon) == 0 {
		return ""
	}
	parameters := " image=" + base64.StdEncoding.EncodeToString(icon)
	if swiftBar {
		parameters += " width=16 height=16"
	}
	return parameters
}

func clean(value string) string {
	value = strings.ReplaceAll(value, "|", " ")
	value = strings.ReplaceAll(value, "\r", " ")
	value = strings.ReplaceAll(value, "\n", " ")
	return strings.TrimSpace(value)
}

func escapeMarkdown(value string) string {
	replacer := strings.NewReplacer("\\", "\\\\", "*", "\\*", "_", "\\_", "`", "\\`")
	return replacer.Replace(value)
}

func quote(value string) string {
	return strconv.Quote(value)
}
