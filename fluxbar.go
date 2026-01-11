package main

import (
	"fmt"
	//"os"
	"strconv"

	miniflux "miniflux.app/client"
)

var MINIFLUX_SERVER string
var MINIFLUX_APIKEY string
var ICON="PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHhtbDpzcGFjZT0icHJlc2VydmUiIHZpZXdCb3g9IjAgNzguOSA1MTIgMzU0LjEiIHdpZHRoPSIyNHB4IiBoZWlnaHQ9IjI0cHgiPjxwYXRoIGZpbGw9IiNmZmYiIGQ9Ik0xNjYuOCA5Ni4yYzE2LjYtOC44IDM1LjItMTMuMiA1NC0xMy4xIDM5LjkgMCA2NS4yIDE3LjMgNzYuMiA1MiAxMi42LTE0LjggMjcuNy0yNy4zIDQ0LjYtMzYuOSAxNy41LTEwLjEgMzctMTUuMSA1OC42LTE1LjEgMjkuOSAwIDUxLjMgOS4xIDY0LjEgMjcuNHMxOS4zIDQ1LjcgMTkuMiA4Mi4zdjIwNi42YzAgNS4yLjcgOC43IDIuMiAxMC41IDEuNCAxLjkgNC42IDMuNiA5LjQgNC45bDE2LjkgNS42VjQzM0g0MTEuN2MtOC43IDAtMTUtMy4zLTE4LjgtOS44LTMuOC02LjYtNS44LTE2LjQtNS44LTI5LjVWMTgwLjFjMC0yMS4xLTIuMy0zNi4xLTctNDVzLTEyLjUtMTMuMy0yMy40LTEzLjRjLTE3LjQgMC0zNS44IDEwLjMtNTUuNCAzMC45IDIuMSAxMy4zIDMgMjYuNyAyLjkgNDAuMXYyMDYuNmMwIDUuMi43IDguNyAyLjIgMTAuNXM0LjYgMy42IDkuNCA0LjlsMTYuOSA1LjZ2MTIuNkgyMzIuNGMtOC43IDAtMTUtMy4zLTE4LjgtOS44cy01LjgtMTYuNC01LjgtMjkuNVYxODAuMWMwLTIxLjEtMi4zLTM2LjEtNy00NXMtMTIuNC0xMy4zLTIzLjQtMTMuNGMtMTcgMC0zNC42IDkuNC01Mi41IDI4LjF2MjQ5LjRjMCA1LjIuNyA4LjggMi4yIDEwLjkgMS40IDIuMSA0LjQgMy45IDguOSA1LjNsMTYuNCA0Ljl2MTIuNkgwdi0xMi42bDE2LjktNS42YzQuOC0xLjQgOC0zIDkuNC00LjlzMi4yLTUuNCAyLjItMTAuNVYxMzMuN2MwLTUuMi0uNy04LjctMi4yLTEwLjUtMS40LTEuOS00LjYtMy41LTkuNC00LjlMMCAxMTIuN1YxMDBsMTE1LjctMjEuMWg4LjJ2NDkuMmMxMi41LTEyLjggMjctMjMuNiA0Mi45LTMxLjkiLz48L3N2Zz4="

type MinifluxFeedEntrie struct {
	EntriID int64
	Title string
	URL   string
	FeedName string
}

func main() {
	//argsWithProg := os.Args
	client := miniflux.New(MINIFLUX_SERVER, MINIFLUX_APIKEY)
	filter := miniflux.Filter{
		Status: miniflux.EntryStatusUnread,
	}
	entries, err := client.Entries(&filter)
	if err != nil {
		return
	}
	entriesList := make([]MinifluxFeedEntrie, 0)
	for _, entry := range entries.Entries {
		entriesList = append(entriesList, MinifluxFeedEntrie{
			EntriID: entry.ID,
			Title: entry.Title,
			URL:   entry.URL,
			FeedName: entry.Feed.Title,
		})
	}

	// Output
	fmt.Println(strconv.Itoa(entries.Total) + " | image=" + ICON);
	fmt.Println("---");
	//fmt.Println(argsWithProg)
	//fmt.Println("**refreh** | md=true refresh=true param1=TEST href=https://test.de")
	for _, entry := range entriesList {
		fmt.Println("**" + entry.FeedName + "**: " + entry.Title + " | href=" + entry.URL + " refresh=true param1=" + strconv.FormatInt(entry.EntriID, 10) + " md=true")
	}
}