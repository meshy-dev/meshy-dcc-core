"""Calling convention for a script ``main`` with one typed parameter."""

from __future__ import annotations

from typing import Any

import pytest

from meshy_dcc_core._server.inprocess_executor import _call_script_main


def test_typed_scalar_parameter_rejects_unknown_keys_before_running() -> None:
    entered = []

    def main(value: str = "") -> Any:
        entered.append(value)
        return value

    with pytest.raises(TypeError, match="unsupported_argument"):
        _call_script_main(main, {"unsupported_argument": True})
    assert entered == []
    assert _call_script_main(main, {"value": "mesh"}) == "mesh"


def test_mapping_parameter_still_receives_the_whole_request() -> None:
    def main(params: dict) -> Any:
        return params

    def untyped(params):
        return params

    assert _call_script_main(main, {"a": 1, "_meta": {}}) == {"a": 1}
    assert _call_script_main(untyped, {"a": 1}) == {"a": 1}
