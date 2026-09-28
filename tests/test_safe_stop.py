"""Tests for the loopback safe-stop endpoint."""

from __future__ import annotations

import json
import urllib.error
import urllib.request

from meshy_dcc_core.safe_stop import serve_safe_stop


def _post(url: str, body: bytes, content_type: str) -> tuple[int, dict]:
    request = urllib.request.Request(url, data=body, headers={"Content-Type": content_type})
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            return response.status, json.loads(response.read())
    except urllib.error.HTTPError as error:
        return error.code, json.loads(error.read())


def test_json_request_reaches_handler_and_form_post_does_not():
    seen = []

    def handler(body):
        seen.append(body)
        return (202, {"stopping": True}) if body.get("discard_unsaved") else (409, {"stopping": False})

    url = serve_safe_stop(handler)
    assert url.startswith("http://127.0.0.1:")

    assert _post(url, b'{"discard_unsaved": false}', "application/json") == (409, {"stopping": False})
    assert _post(url, b'{"discard_unsaved": true}', "application/json") == (202, {"stopping": True})
    status, _ = _post(url, b"discard_unsaved=true", "application/x-www-form-urlencoded")
    assert status == 415
    assert seen == [{"discard_unsaved": False}, {"discard_unsaved": True}]
