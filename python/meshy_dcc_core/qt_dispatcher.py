"""Compatibility import for :mod:`meshy_dcc_core.host.qt_dispatcher`.

New integrations should import the dispatcher from the public ``host``
namespace.  This module remains for adapters released before that namespace
became canonical.
"""

from __future__ import annotations

from meshy_dcc_core.host.qt_dispatcher import DISPATCHER_VERSION
from meshy_dcc_core.host.qt_dispatcher import QtCommandServer
from meshy_dcc_core.host.qt_dispatcher import ServerHandle
from meshy_dcc_core.host.qt_dispatcher import current_server
from meshy_dcc_core.host.qt_dispatcher import start_qt_server
from meshy_dcc_core.host.qt_dispatcher import stop_qt_server

__all__ = [
    "DISPATCHER_VERSION",
    "QtCommandServer",
    "ServerHandle",
    "current_server",
    "start_qt_server",
    "stop_qt_server",
]
