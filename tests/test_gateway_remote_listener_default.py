"""The embedded gateway serves loopback only unless a remote port is configured.

The remote listener has no authentication; binding it by default would let
anyone on the network call DCC tools.
"""

from meshy_dcc_core import McpHttpConfig


def test_remote_listener_is_off_by_default(monkeypatch):
    monkeypatch.delenv("MESHY_DCC_GATEWAY_REMOTE_PORT", raising=False)
    monkeypatch.delenv("MESHY_DCC_GATEWAY_REMOTE_HOST", raising=False)
    config = McpHttpConfig(port=0)
    assert config.gateway_remote_port == 0


def test_remote_listener_is_opt_in(monkeypatch):
    monkeypatch.setenv("MESHY_DCC_GATEWAY_REMOTE_PORT", "59765")
    monkeypatch.setenv("MESHY_DCC_GATEWAY_REMOTE_HOST", "0.0.0.0")
    config = McpHttpConfig(port=0)
    assert config.gateway_remote_port == 59765
    assert config.gateway_remote_host == "0.0.0.0"
