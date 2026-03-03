from http.server import HTTPServer, BaseHTTPRequestHandler
import json
import threading
import urllib.request

API_PORT = 8083
WEB_PORT = 8084

ITEMS = [
    {"id": 1, "name": "raspberry-pi"},
    {"id": 2, "name": "nas-drive"},
]


class ApiHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/api/items":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"items": ITEMS}).encode())
        elif self.path == "/api/health":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"api": "ok"}).encode())
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        print(f"[API:{API_PORT}] {args[0]}", flush=True)


class WebHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/":
            try:
                with urllib.request.urlopen(f"http://localhost:{API_PORT}/api/items") as response:
                    data = json.loads(response.read())
                items_html = "".join(
                    f"<li>{item['name']} (id: {item['id']})</li>"
                    for item in data["items"]
                )
            except Exception as error:
                items_html = f"<li>Error fetching from API: {error}</li>"

            html = f"""\
<!DOCTYPE html>
<html>
<head><title>Fullstack App</title></head>
<body>
  <h1>Devices</h1>
  <ul>{items_html}</ul>
  <p>Data fetched from API at port {API_PORT}</p>
</body>
</html>"""
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(html.encode())
        else:
            self.send_response(404)
            self.end_headers()

    def log_message(self, format, *args):
        print(f"[WEB:{WEB_PORT}] {args[0]}", flush=True)


if __name__ == "__main__":
    api_server = HTTPServer(("0.0.0.0", API_PORT), ApiHandler)
    web_server = HTTPServer(("0.0.0.0", WEB_PORT), WebHandler)

    api_thread = threading.Thread(target=api_server.serve_forever, daemon=True)
    api_thread.start()
    print(f"API server listening on port {API_PORT}", flush=True)

    print(f"Web server listening on port {WEB_PORT}", flush=True)
    web_server.serve_forever()
