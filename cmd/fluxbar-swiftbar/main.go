package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/KevinCFechtel/FluxBar/internal/assets"
	"github.com/KevinCFechtel/FluxBar/internal/config"
	"github.com/KevinCFechtel/FluxBar/internal/icons"
	fluxminiflux "github.com/KevinCFechtel/FluxBar/internal/miniflux"
	"github.com/KevinCFechtel/FluxBar/internal/swiftbar"
)

var MINIFLUX_SERVER string
var MINIFLUX_APIKEY string

func main() {
	logger := newLogger()
	resolved, err := config.Resolve(MINIFLUX_SERVER, MINIFLUX_APIKEY)
	if err != nil {
		printError(err)
		logger.Print(err)
		return
	}
	service := fluxminiflux.New(resolved.Server, resolved.APIKey, logger)

	shellPath := ""
	if len(os.Args) > 1 {
		shellPath = os.Args[1]
	}
	if len(os.Args) > 2 && os.Args[2] != "" {
		entryID, parseErr := strconv.ParseInt(os.Args[2], 10, 64)
		if parseErr != nil {
			logger.Printf("Ungültige Entry-ID %q: %v", os.Args[2], parseErr)
		} else {
			ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
			if markErr := service.MarkRead(ctx, entryID); markErr != nil {
				logger.Print(markErr)
			}
			cancel()
		}
	}

	ctx, cancel := context.WithTimeout(context.Background(), 45*time.Second)
	defer cancel()
	entries, total, err := service.Unread(ctx)
	if err != nil {
		printError(err)
		logger.Print(err)
		return
	}
	titleIcon, err := icons.Normalize(assets.MinifluxSVG(), "image/svg+xml", icons.DefaultSize)
	if err != nil {
		logger.Printf("Menüleisten-Icon konnte nicht verarbeitet werden: %v", err)
	}
	darkMode := strings.EqualFold(os.Getenv("OS_APPEARANCE"), "Dark")
	if debugIcons, _ := strconv.ParseBool(os.Getenv("FLUXBAR_DEBUG_ICONS")); debugIcons {
		logger.Printf(
			"level=debug component=swiftbar event=appearance_selected os_appearance=%q dark=%t",
			os.Getenv("OS_APPEARANCE"), darkMode,
		)
	}
	if err := swiftbar.Render(os.Stdout, entries, total, swiftbar.Options{
		ShellPath: shellPath,
		SwiftBar:  os.Getenv("SWIFTBAR") == "1",
		DarkMode:  darkMode,
		TitleIcon: titleIcon,
	}); err != nil {
		logger.Printf("SwiftBar-Ausgabe fehlgeschlagen: %v", err)
	}
}

func newLogger() *log.Logger {
	executable, err := os.Executable()
	if err != nil {
		return log.New(os.Stderr, "", log.LstdFlags|log.Lmicroseconds)
	}
	logFile, err := os.OpenFile(filepath.Join(filepath.Dir(executable), "fluxbar.log"), os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o600)
	if err != nil {
		return log.New(os.Stderr, "", log.LstdFlags|log.Lmicroseconds)
	}
	return log.New(logFile, "", log.LstdFlags|log.Lmicroseconds)
}

func printError(err error) {
	fmt.Println("! | color=red")
	fmt.Println("---")
	fmt.Printf("FluxBar: %s\n", err)
}
