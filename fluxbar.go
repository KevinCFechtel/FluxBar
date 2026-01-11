package main

import (
	"fmt"
	"log"
	"os"
	"path"
	"path/filepath"
	"strconv"

	miniflux "miniflux.app/v2/client"
)

var MINIFLUX_SERVER string
var MINIFLUX_APIKEY string
var ICON="PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHhtbDpzcGFjZT0icHJlc2VydmUiIHZpZXdCb3g9IjAgNzguOSA1MTIgMzU0LjEiIHdpZHRoPSIyNHB4IiBoZWlnaHQ9IjI0cHgiPjxwYXRoIGZpbGw9IiNmZmYiIGQ9Ik0xNjYuOCA5Ni4yYzE2LjYtOC44IDM1LjItMTMuMiA1NC0xMy4xIDM5LjkgMCA2NS4yIDE3LjMgNzYuMiA1MiAxMi42LTE0LjggMjcuNy0yNy4zIDQ0LjYtMzYuOSAxNy41LTEwLjEgMzctMTUuMSA1OC42LTE1LjEgMjkuOSAwIDUxLjMgOS4xIDY0LjEgMjcuNHMxOS4zIDQ1LjcgMTkuMiA4Mi4zdjIwNi42YzAgNS4yLjcgOC43IDIuMiAxMC41IDEuNCAxLjkgNC42IDMuNiA5LjQgNC45bDE2LjkgNS42VjQzM0g0MTEuN2MtOC43IDAtMTUtMy4zLTE4LjgtOS44LTMuOC02LjYtNS44LTE2LjQtNS44LTI5LjVWMTgwLjFjMC0yMS4xLTIuMy0zNi4xLTctNDVzLTEyLjUtMTMuMy0yMy40LTEzLjRjLTE3LjQgMC0zNS44IDEwLjMtNTUuNCAzMC45IDIuMSAxMy4zIDMgMjYuNyAyLjkgNDAuMXYyMDYuNmMwIDUuMi43IDguNyAyLjIgMTAuNXM0LjYgMy42IDkuNCA0LjlsMTYuOSA1LjZ2MTIuNkgyMzIuNGMtOC43IDAtMTUtMy4zLTE4LjgtOS44cy01LjgtMTYuNC01LjgtMjkuNVYxODAuMWMwLTIxLjEtMi4zLTM2LjEtNy00NXMtMTIuNC0xMy4zLTIzLjQtMTMuNGMtMTcgMC0zNC42IDkuNC01Mi41IDI4LjF2MjQ5LjRjMCA1LjIuNyA4LjggMi4yIDEwLjkgMS40IDIuMSA0LjQgMy45IDguOSA1LjNsMTYuNCA0Ljl2MTIuNkgwdi0xMi42bDE2LjktNS42YzQuOC0xLjQgOC0zIDkuNC00LjlzMi4yLTUuNCAyLjItMTAuNVYxMzMuN2MwLTUuMi0uNy04LjctMi4yLTEwLjUtMS40LTEuOS00LjYtMy41LTkuNC00LjlMMCAxMTIuN1YxMDBsMTE1LjctMjEuMWg4LjJ2NDkuMmMxMi41LTEyLjggMjctMjMuNiA0Mi45LTMxLjkiLz48L3N2Zz4="

type MinifluxFeedEntrie struct {
	EntryID int64
	Title string
	URL   string
	FeedName string
}

func main() {
	argsWithProg := os.Args
	shellPath := ""

	// Miniflux Client
	client := miniflux.New(MINIFLUX_SERVER, MINIFLUX_APIKEY)
	filter := miniflux.Filter{
		Status: miniflux.EntryStatusUnread,
		Order:  "published_at",
		Direction: "asc",
	}

	// Setup logging
	dir, _ := filepath.Split(argsWithProg[0])
    logpath := path.Join(dir, "fluxbar.log")
	logFile, err := os.OpenFile(logpath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0666)
    if err != nil {
        log.Fatalf("Failed to open log file: %v", err)
    }
    defer logFile.Close()
    log.SetOutput(logFile)
    log.SetFlags(log.LstdFlags)

	entryIDs := make([]int64,0)
	if len(argsWithProg) > 1 {
		shellPath = argsWithProg[1]
		if len(argsWithProg) > 2 {
			entryID := argsWithProg[2]
			if entryID != "" {
				log.Println("Recieved EntryID: ", entryID)
				id, err := strconv.ParseInt(entryID, 10, 64)
				if err == nil {
					entryIDs = append(entryIDs, id)
				}
			}
		}
	}
	if len(entryIDs) > 0 {
		//client.UpdateEntries(entryIDs, miniflux.EntryStatusRead)
	}

	entriesList := make([]MinifluxFeedEntrie, 0)
	entriesCount := 0
	entries, err := client.Entries(&filter)
	if err == nil {
		entriesCount = entries.Total
		for _, entry := range entries.Entries {
			entriesList = append(entriesList, MinifluxFeedEntrie{
				EntryID: entry.ID,
				Title: entry.Title,
				URL:   entry.URL,
				FeedName: entry.Feed.Title,
			})
		}
	}

	// Output
	fmt.Println(strconv.Itoa(entriesCount) + " | image=" + ICON);
	fmt.Println("---");
	for _, entry := range entriesList {
		fmt.Println("**" + entry.FeedName + "**: " + entry.Title + " | bash=" + shellPath + " refresh=true param1=" + strconv.FormatInt(entry.EntryID, 10) + " terminal=false md=true ") //href=" + entry.URL
	}
}