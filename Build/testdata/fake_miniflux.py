#!/usr/bin/env python3
"""Deterministic fake Miniflux v1 server for differential adapter tests."""
import json
import re
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

BASE_TS = "2026-08-22T10:00:00Z"

def entry(eid, status="unread", starred=False):
    category_id = 5 if eid % 10 == 0 else 2
    category_title = "News" if category_id == 5 else "Tech"
    return {
        "id": eid,
        "feed_id": 3 if eid % 10 else 4,
        "title": f"Entry {eid}",
        "url": f"https://example.com/{eid}",
        "comments_url": "",
        "status": status,
        "starred": starred,
        "published_at": BASE_TS,
        "content": "",
        "feed": {
            "id": 3 if eid % 10 else 4,
            "title": "Alpha Feed" if eid % 10 else "Beta Feed",
            "category": {"id": category_id, "title": category_title},
        },
    }

ALL_IDS = list(range(1, 206))            # 205 entries, mixed statuses
for i in ALL_IDS:
    pass
STATES = {eid: ("read" if eid % 7 == 0 else "unread") for eid in ALL_IDS}
STARRED = {eid for eid in ALL_IDS if eid % 11 == 0}

UNREAD_IDS = [e for e in ALL_IDS if STATES[e] == "unread"]

def paged(ids, query):
    m = re.search(r"after_entry_id=(\d+)", query)
    after = int(m.group(1)) if m else 0
    page = [e for e in ids if e > after][:200]
    body = {
        "total": len(ids),
        "entries": [
            entry(e, STATES[e], e in STARRED) for e in page
        ],
    }
    return json.dumps(body).encode()

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):  # silence
        pass

    def do_GET(self):
        path, _, query = self.path.partition("?")
        if path == "/v1/feeds/counters":
            out = json.dumps({"reads": {}, "unreads": {"3": 150, "4": 20}}).encode()
        elif path == "/v1/categories":
            out = json.dumps([
                {"id": 2, "title": "Tech"},
                {"id": 5, "title": "News"},
            ]).encode()
        elif path == "/v1/feeds":
            # Feed 9 references a missing category (orphan); feed 4 has none.
            out = json.dumps([
                {"id": 3, "title": "Alpha Feed", "category": {"id": 2, "title": "Tech"}},
                {"id": 4, "title": "beta feed", "category": None},
                {"id": 9, "title": "Orphan", "category": {"id": 99, "title": "Ghost"}},
            ]).encode()
        elif path.startswith("/v1/entries"):
            from urllib.parse import parse_qs
            params = parse_qs(query)
            if params.get("starred") == ["1"]:
                ids = sorted(STARRED)
            elif params.get("status") == ["unread"]:
                ids = UNREAD_IDS
            else:
                ids = ALL_IDS
            if "category_id" in params:
                category_id = int(params["category_id"][0])
                ids = [eid for eid in ids if (5 if eid % 10 == 0 else 2) == category_id]
            if "feed_id" in params:
                feed_id = int(params["feed_id"][0])
                ids = [eid for eid in ids if (3 if eid % 10 else 4) == feed_id]
            out = paged(ids, query)
        else:
            self.send_response(404); self.end_headers(); return
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

if __name__ == "__main__":
    port = int(sys.argv[1])
    HTTPServer(("127.0.0.1", port), Handler).serve_forever()
