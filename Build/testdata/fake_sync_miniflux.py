#!/usr/bin/env python3
"""Stateful fake Miniflux used only by Phase 8 differential tests."""
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlsplit

PORT = int(sys.argv[1])
MODE = sys.argv[2]
LOG_PATH = sys.argv[3]

entries = {
    1: {
        "id": 1, "feed_id": 20, "title": "One", "url": "https://example.com/1",
        "comments_url": "", "status": "unread", "starred": False,
        "published_at": "2026-08-22T10:00:01Z",
        "content": "<p>First article with <strong>bold text</strong> and a break.<br>Second line.</p><img src=\"/inline.jpg\" alt=\"Inline photo\">",
        "enclosures": [],
        "feed": {"id": 20, "title": "Feed", "category": {"id": 10, "title": "Category"}},
    },
    2: {
        "id": 2, "feed_id": 20, "title": "Two", "url": "https://example.com/2",
        "comments_url": "", "status": "unread", "starred": False,
        "published_at": "2026-08-22T10:00:02Z",
        "content": "<p>Second article relies on an enclosure fallback.</p>",
        "enclosures": [
            {"id": 1, "url": "https://example.com/audio.mp3", "mime_type": "audio/mpeg"},
            {"id": 2, "url": "/cover.jpg", "mime_type": "image/jpeg"},
        ],
        "feed": {"id": 20, "title": "Feed", "category": {"id": 10, "title": "Category"}},
    },
}
mutation_count = 0
counters_count = 0
selection_count = 0

def send(handler, status, value=None):
    body = b"" if value is None else json.dumps(value, separators=(",", ":")).encode()
    handler.send_response(status)
    if body:
        handler.send_header("Content-Type", "application/json")
        handler.send_header("Content-Length", str(len(body)))
    handler.end_headers()
    if body:
        handler.wfile.write(body)

def log(method, path, body):
    with open(LOG_PATH, "a", encoding="utf-8") as output:
        output.write(json.dumps({"method": method, "path": path, "body": body}, sort_keys=True) + "\n")

def read_body(handler):
    if handler.headers.get("Transfer-Encoding", "").lower() == "chunked":
        chunks = []
        while True:
            size = int(handler.rfile.readline().strip(), 16)
            if size == 0:
                handler.rfile.readline()
                break
            chunks.append(handler.rfile.read(size))
            handler.rfile.read(2)
        return b"".join(chunks).decode()
    length = int(handler.headers.get("Content-Length", "0"))
    return handler.rfile.read(length).decode() if length else ""

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        global counters_count, selection_count
        split = urlsplit(self.path)
        log("GET", self.path, None)
        if split.path == "/v1/feeds/counters":
            counters_count += 1
            if MODE == "refresh-5xx" and counters_count == 2:
                return send(self, 500, {"error_message": "refresh failed"})
            if MODE == "refresh-auth" and counters_count == 2:
                return send(self, 401)
            if MODE == "incremental" and counters_count == 2:
                entries[1]["status"] = "read"
                entries[1]["starred"] = True
                entries[3] = {"id": 3, "feed_id": 20, "title": "Three", "url": "https://example.com/3", "comments_url": "", "status": "unread", "starred": False, "published_at": "2026-08-22T10:00:03Z", "content": "", "feed": {"id": 20, "title": "Feed", "category": {"id": 10, "title": "Category"}}}
            unread = sum(value["status"] == "unread" for value in entries.values())
            return send(self, 200, {"reads": {}, "unreads": {"20": unread}})
        if split.path == "/v1/categories":
            return send(self, 200, [{"id": 10, "title": "Category"}])
        if split.path == "/v1/feeds":
            return send(self, 200, [{"id": 20, "title": "Feed", "category": {"id": 10, "title": "Category"}}])
        if split.path.startswith("/v1/entries/"):
            entry_id = int(split.path.rsplit("/", 1)[1])
            return send(self, 200, entries[entry_id])
        if split.path == "/v1/entries":
            query = parse_qs(split.query)
            selected = list(entries.values())
            statuses = query.get("status", [])
            if statuses:
                selected = [value for value in selected if value["status"] in statuses]
            if query.get("starred") == ["1"]:
                selected = [value for value in selected if value["starred"]]
            after = int(query.get("after_entry_id", [0])[0])
            selected = [value for value in selected if value["id"] > after]
            selected.sort(key=lambda value: value["id"])
            total = len(selected)
            limit = int(query.get("limit", [len(selected) or 1])[0])
            if query.get("order") == ["id"]:
                selection_count += 1
                if MODE == "incomplete" and selection_count == 2:
                    total += 1
            return send(self, 200, {"total": total, "entries": selected[:limit]})
        return send(self, 404)

    def do_PUT(self):
        global mutation_count
        raw = read_body(self)
        body = json.loads(raw) if raw else None
        log("PUT", self.path, body)
        mutation_count += 1
        if MODE == "fail-first" and mutation_count == 1:
            return send(self, 500, {"error_message": "scripted failure"})
        if MODE == "fail-second" and mutation_count == 2:
            return send(self, 500, {"error_message": "scripted failure"})
        if self.path == "/v1/entries":
            for entry_id in body["entry_ids"]:
                entries[entry_id]["status"] = body["status"]
            return send(self, 204)
        if self.path.endswith("/star"):
            entry_id = int(self.path.split("/")[-2])
            entries[entry_id]["starred"] = not entries[entry_id]["starred"]
            return send(self, 204)
        return send(self, 404)

HTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
