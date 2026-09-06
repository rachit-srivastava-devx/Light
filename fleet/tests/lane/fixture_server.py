#!/usr/bin/env python3
import json
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        self.rfile.read(length)
        if self.path == "/busy":
            body, status = {"error": {"message": "model is busy"}}, 200
        elif self.path == "/rate":
            body, status = {"error": {"message": "rate limited"}}, 429
        elif self.path == "/timeout":
            time.sleep(2)
            body, status = {"error": {"message": "late response"}}, 503
        elif self.path == "/down":
            body, status = {"error": {"message": "temporary failure"}}, 503
        else:
            body = {"model": "served-by-second-lane", "choices": [{"message": {"content": "SECOND_LANE_OK"}}]}
            status = 200
        encoded = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        if self.path == "/rate":
            self.send_header("Retry-After", "1")
        self.end_headers()
        self.wfile.write(encoded)

    def log_message(self, *_args):
        pass


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
print(server.server_port, flush=True)
server.serve_forever()
