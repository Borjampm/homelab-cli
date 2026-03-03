from http.server import HTTPServer, BaseHTTPRequestHandler
import json


class ApiHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/health":
            self.send_json(200, {"status": "ok"})
        elif self.path == "/items":
            self.send_json(200, {
                "items": [
                    {"id": 1, "name": "keyboard"},
                    {"id": 2, "name": "monitor"},
                    {"id": 3, "name": "mouse"},
                ]
            })
        elif self.path.startswith("/items/"):
            item_id = self.path.split("/")[-1]
            self.send_json(200, {"id": int(item_id), "name": f"item-{item_id}"})
        else:
            self.send_json(404, {"error": "not found"})

    def do_POST(self):
        if self.path == "/items":
            content_length = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(content_length))
            self.send_json(201, {"id": 99, "name": body.get("name", "unnamed")})
        else:
            self.send_json(404, {"error": "not found"})

    def send_json(self, status, data):
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def log_message(self, format, *args):
        print(f"[API] {args[0]}")


if __name__ == "__main__":
    port = 8081
    server = HTTPServer(("0.0.0.0", port), ApiHandler)
    print(f"API server listening on port {port}", flush=True)
    server.serve_forever()
