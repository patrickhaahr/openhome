#!/usr/bin/env python3
"""Forward the emulator's traffic to a local fixture API, losing some Workout create responses.

The fixture API receives and commits each lost create; the client only sees its connection cut, which
is the outcome a duplicate-safe retry has to survive. Everything else passes through unchanged, and a
stopped API looks unreachable. It only ever forwards to loopback; the emulator reaches it via 10.0.2.2.
"""

import argparse
import http.client
import http.server
import threading

UPSTREAM = ("127.0.0.1", 8000)
HOP_BY_HOP = {"connection", "transfer-encoding", "keep-alive"}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lose-workout-responses", type=int, default=1)
    parser.add_argument("--port", type=int, default=8001)
    args = parser.parse_args()

    lock = threading.Lock()
    remaining = [args.lose_workout_responses]

    def lose_this_response() -> bool:
        with lock:
            if remaining[0] <= 0:
                return False
            remaining[0] -= 1
            return True

    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def forward(self) -> None:
            body = self.rfile.read(int(self.headers.get("Content-Length") or 0))
            connection = http.client.HTTPConnection(*UPSTREAM, timeout=30)
            try:
                connection.request(self.command, self.path, body, dict(self.headers))
                response = connection.getresponse()
                payload = response.read()
            except OSError as error:
                print(f"{self.command} {self.path}: API unreachable ({error}); cutting the connection", flush=True)
                self.close_connection = True
                return
            finally:
                connection.close()

            print(f"{self.command} {self.path}: API answered {response.status}", flush=True)
            if self.command == "POST" and self.path == "/api/workouts" and lose_this_response():
                print("  response lost: cutting the connection", flush=True)
                self.close_connection = True
                return
            self.send_response_only(response.status)
            for name, value in response.getheaders():
                if name.lower() not in HOP_BY_HOP:
                    self.send_header(name, value)
            self.end_headers()
            self.wfile.write(payload)

        do_GET = do_POST = do_PATCH = do_PUT = do_DELETE = forward

        def log_message(self, format, *args) -> None:
            pass

    server = http.server.ThreadingHTTPServer(("0.0.0.0", args.port), Handler)
    print(f"Forwarding :{args.port} to {UPSTREAM[0]}:{UPSTREAM[1]}, losing {remaining[0]} Workout responses", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
