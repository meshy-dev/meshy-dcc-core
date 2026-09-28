"""Import-light Rez deployment and sidecar lifecycle API.

The implementation remains available from ``meshy_dcc_core.install_lifecycle``
for compatibility.  New code should use this ownership-oriented namespace.
"""

from __future__ import annotations

from meshy_dcc_core.deployment.install_sop import INSTALL_EXIT_ACQUIRE
from meshy_dcc_core.deployment.install_sop import INSTALL_EXIT_CODES
from meshy_dcc_core.deployment.install_sop import INSTALL_EXIT_INSTALL
from meshy_dcc_core.deployment.install_sop import INSTALL_EXIT_OK
from meshy_dcc_core.deployment.install_sop import INSTALL_EXIT_PREFLIGHT
from meshy_dcc_core.deployment.install_sop import INSTALL_EXIT_REQUIRES_RESTART
from meshy_dcc_core.deployment.install_sop import INSTALL_EXIT_VERIFY
from meshy_dcc_core.deployment.install_sop import INSTALL_SOP_SCHEMA_VERSION
from meshy_dcc_core.deployment.install_sop import load_install_sop_schema
from meshy_dcc_core.deployment.install_sop import validate_install_sop_report
from meshy_dcc_core.install_lifecycle import *  # noqa: F403
from meshy_dcc_core.install_lifecycle import __all__ as _LIFECYCLE_EXPORTS

__all__ = [
    *_LIFECYCLE_EXPORTS,
    "INSTALL_EXIT_ACQUIRE",
    "INSTALL_EXIT_CODES",
    "INSTALL_EXIT_INSTALL",
    "INSTALL_EXIT_OK",
    "INSTALL_EXIT_PREFLIGHT",
    "INSTALL_EXIT_REQUIRES_RESTART",
    "INSTALL_EXIT_VERIFY",
    "INSTALL_SOP_SCHEMA_VERSION",
    "load_install_sop_schema",
    "validate_install_sop_report",
]
