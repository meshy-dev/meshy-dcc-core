"""Loopback endpoint an adapter can advertise as ``safe_stop_url``.

``meshy-dcc stop-instance`` POSTs ``{"instance_id", "dcc_type", "owner",
"session", "discard_unsaved"}`` as JSON to the URL in an instance's
``safe_stop_url`` metadata. The adapter's handler decides whether the host may
close; this module only serves the request on 127.0.0.1.
"""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler
from http.server import ThreadingHTTPServer
from typing import Any
from typing import Callable
from typing import Dict
from typing import Tuple

SafeStopHandler = Callable[[Dict[str, Any]], Tuple[int, Dict[str, Any]]]


def serve_safe_stop(handler: SafeStopHandler) -> str:
    """Serve ``handler`` for POST on a free loopback port and return its URL.

    Only ``application/json`` bodies reach the handler, so a web page cannot
    trigger a stop with a plain cross-origin form post.
    """

    class _Request(BaseHTTPRequestHandler):
        def do_POST(self) -> None:  # noqa: N802 - http.server naming
            if not (self.headers.get("Content-Type") or "").startswith("application/json"):
                status, reply = 415, {"stopping": False, "error": "expected application/json"}
            else:
                try:
                    length = int(self.headers.get("Content-Length") or 0)
                    body = json.loads(self.rfile.read(length) or b"{}")
                    status, reply = handler(body if isinstance(body, dict) else {})
                except Exception as exc:  # noqa: BLE001 - a bad request must not reach the host
                    status, reply = 500, {"stopping": False, "error": str(exc)}
            payload = json.dumps(reply).encode("utf-8")
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, *args: Any) -> None:
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), _Request)
    threading.Thread(target=server.serve_forever, name="dcc-mcp-safe-stop", daemon=True).start()
    return "http://127.0.0.1:{}/safe-stop".format(server.server_address[1])


__all__ = ["SafeStopHandler", "serve_safe_stop"]
