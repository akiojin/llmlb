#!/usr/bin/env python3

import argparse
import json
import uuid
from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    server_version = "llm-router-poc/0.1"

    def _send_json(self, status: int, payload: dict) -> None:
        data = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, format: str, *args) -> None:
        # Quiet by default (the runner script prints high-level progress).
        return

    def do_GET(self) -> None:
        if self.path == "/v1/models":
            self._send_json(200, {"object": "list", "data": []})
            return
        self._send_json(404, {"error": {"message": f"not found: {self.path}"}})

    def do_POST(self) -> None:
        if self.path == "/api/nodes":
            # Minimal node registration response expected by node/src/api/router_client.cpp
            self._send_json(
                200,
                {"node_id": str(uuid.uuid4()), "agent_token": str(uuid.uuid4())},
            )
            return

        if self.path == "/api/health":
            self._send_json(200, {"ok": True})
            return

        self._send_json(404, {"error": {"message": f"not found: {self.path}"}})


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=18080)
    args = ap.parse_args()

    httpd = HTTPServer((args.host, args.port), Handler)
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        return 0
    finally:
        httpd.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

