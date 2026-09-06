#!/usr/bin/env python3
"""A stand-in for the gateway-sidecar's `/v1/complete` contract (lane contract §3.7).

Used by `tests/test_f04_build_mode_wire.py`'s A1 (a real relay-py process, driven over real HTTP)
and by the lane contract's §6 manual E2E. Stubs the upstream MODEL while the real RELAY runs,
because the claim under test is which system prompt the relay sends, not what a model replies --
`relay-rs/src/provider.rs:736-877` already establishes this exact idiom in this codebase (raw
TcpStream HTTP stubs).

Records every request body it receives to a JSONL file (one line per request) so a test or an
operator can inspect exactly what reached the model boundary: "the relay returned 200" is a proxy;
"the build prompt reached the model boundary" is the property.

Not a test file itself (pytest's default `test_*.py` collection pattern does not match this name),
and not production code -- see `docs/lane-contracts/F04-server-side-build-mode.md` §3.7.
"""

from __future__ import annotations

import argparse
import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

_STUB_REPLY_TEXT = "Understood. What detail comes next?"
_write_lock = threading.Lock()


def _make_handler(record_path: str) -> type[BaseHTTPRequestHandler]:
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, format_: str, *args: object) -> None:
            pass  # keep the manual drive's terminal output readable; failures still 4xx/5xx below

        def do_GET(self) -> None:
            if self.path == "/healthz":
                self._reply(200, {"status": "ok"})
                return
            self._reply(404, {"error": "NOT_FOUND"})

        def do_POST(self) -> None:
            if self.path != "/v1/complete":
                self._reply(404, {"error": "NOT_FOUND"})
                return
            length = int(self.headers.get("Content-Length", "0"))
            raw = self.rfile.read(length) if length else b"{}"
            try:
                body = json.loads(raw.decode("utf-8"))
            except json.JSONDecodeError:
                self._reply(400, {"error": "BAD_REQUEST", "message": "invalid JSON"})
                return

            with _write_lock, open(record_path, "a", encoding="utf-8") as record_file:
                record_file.write(json.dumps(body) + "\n")

            self._reply(
                200,
                {
                    "content": [{"type": "text", "text": _STUB_REPLY_TEXT}],
                    "usage": {"input_tokens": 12, "output_tokens": 8},
                    "model": "stub-model",
                    "adapter": "stub",
                    "is_fake_adapter": True,
                },
            )

        def _reply(self, status: int, payload: dict[str, object]) -> None:
            body = json.dumps(payload).encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    return Handler


def run(port: int, record_path: str) -> None:
    open(record_path, "a", encoding="utf-8").close()  # create if absent; never truncate on restart
    server = ThreadingHTTPServer(("127.0.0.1", port), _make_handler(record_path))
    server.serve_forever()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--record", required=True, help="JSONL file to append every request body to")
    args = parser.parse_args()
    run(args.port, args.record)
