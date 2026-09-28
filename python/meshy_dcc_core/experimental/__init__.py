"""Explicit namespace for supported but non-stable Python APIs.

Experimental symbols may change outside the major-version compatibility
window.  They remain reachable from the package root for one compatibility
cycle but are intentionally excluded from its ``__all__``.
"""

from __future__ import annotations

from meshy_dcc_core._exports import _EXPERIMENTAL_LAZY
from meshy_dcc_core._lazy import lazy_dir
from meshy_dcc_core._lazy import resolve_lazy_symbol

__all__ = sorted(_EXPERIMENTAL_LAZY)


def __getattr__(name: str) -> object:
    return resolve_lazy_symbol(name, _EXPERIMENTAL_LAZY, module_name=__name__)


def __dir__() -> list[str]:
    return lazy_dir(_EXPERIMENTAL_LAZY)
