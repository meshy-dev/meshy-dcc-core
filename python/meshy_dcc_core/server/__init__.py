"""Public server construction and host-dispatch contracts.

This namespace owns the adapter-facing server API.  The historical
``meshy_dcc_core._server`` package remains an implementation detail and a
compatibility import for existing adapters.
"""

from __future__ import annotations

from meshy_dcc_core._server import *  # noqa: F403
from meshy_dcc_core._server import __all__ as _SERVER_EXPORTS
from meshy_dcc_core._server.diagnostic_state import DiagnosticRuntimeState
from meshy_dcc_core._server.gateway_guardian import GatewayDaemonGuardian
from meshy_dcc_core._server.gateway_guardian import build_gateway_daemon_command
from meshy_dcc_core._server.gateway_guardian import ensure_gateway_daemon
from meshy_dcc_core._server.gateway_guardian import launch_gateway_daemon

__all__ = [
    *_SERVER_EXPORTS,
    "DiagnosticRuntimeState",
    "GatewayDaemonGuardian",
    "build_gateway_daemon_command",
    "ensure_gateway_daemon",
    "launch_gateway_daemon",
]
