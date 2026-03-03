import http.server
import socket
import platform

PORT = 8000


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = (
            f"hostname:  {socket.gethostname()}\n"
            f"platform:  {platform.system()} {platform.release()}\n"
            f"python3:    {platform.python_version()}\n"
        ).encode()

        self.send_response(200)
        self.send_header("Content-Type", "text/plain")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        print(f"request from {self.address_string()}: {format % args}")


print(f"serving on http://0.0.0.0:{PORT}")
http.server.HTTPServer(("0.0.0.0", PORT), Handler).serve_forever()
