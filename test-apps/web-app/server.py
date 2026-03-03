from http.server import HTTPServer, BaseHTTPRequestHandler

INDEX_HTML = """\
<!DOCTYPE html>
<html>
<head><title>Homelab Web App</title></head>
<body>
  <h1>Welcome to the Homelab</h1>
  <p>This is a web application running on a remote node.</p>
  <ul>
    <li><a href="/about">About</a></li>
    <li><a href="/status">Status</a></li>
  </ul>
</body>
</html>"""

ABOUT_HTML = """\
<!DOCTYPE html>
<html>
<head><title>About</title></head>
<body>
  <h1>About</h1>
  <p>A test web app for homelab-cli integration testing.</p>
</body>
</html>"""

STATUS_HTML = """\
<!DOCTYPE html>
<html>
<head><title>Status</title></head>
<body>
  <h1>Status: Running</h1>
  <p>Server is healthy.</p>
</body>
</html>"""


class WebHandler(BaseHTTPRequestHandler):
    ROUTES = {
        "/": INDEX_HTML,
        "/about": ABOUT_HTML,
        "/status": STATUS_HTML,
    }

    def do_GET(self):
        page = self.ROUTES.get(self.path)
        if page:
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(page.encode())
        else:
            self.send_response(404)
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(b"<h1>404 Not Found</h1>")

    def log_message(self, format, *args):
        print(f"[WEB] {args[0]}")


if __name__ == "__main__":
    port = 8082
    server = HTTPServer(("0.0.0.0", port), WebHandler)
    print(f"Web server listening on port {port}", flush=True)
    server.serve_forever()
