from http.server import HTTPServer, BaseHTTPRequestHandler
import json

PORT = 8085


def handle_rpc(method, params):
    if method == "add":
        return params[0] + params[1]
    elif method == "multiply":
        return params[0] * params[1]
    elif method == "echo":
        return params
    elif method == "server.info":
        return {"name": "homelab-rpc", "version": "1.0", "methods": ["add", "multiply", "echo", "server.info"]}
    else:
        raise ValueError(f"unknown method: {method}")


class RpcHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(content_length))

        request_id = body.get("id")
        method = body.get("method", "")
        params = body.get("params", [])

        try:
            result = handle_rpc(method, params)
            response = {"jsonrpc": "2.0", "result": result, "id": request_id}
        except Exception as error:
            response = {
                "jsonrpc": "2.0",
                "error": {"code": -32601, "message": str(error)},
                "id": request_id,
            }

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(response).encode())

    def log_message(self, format, *args):
        print(f"[RPC] {args[0]}", flush=True)


if __name__ == "__main__":
    server = HTTPServer(("0.0.0.0", PORT), RpcHandler)
    print(f"JSON-RPC server listening on port {PORT}", flush=True)
    server.serve_forever()
