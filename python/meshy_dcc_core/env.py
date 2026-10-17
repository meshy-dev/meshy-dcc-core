"""Import-light environment parsing helpers."""

# ruff: noqa: UP045

from __future__ import annotations

import os
from pathlib import Path
from typing import Any
from typing import Iterable
from typing import Optional

from meshy_dcc_core.constants import ENV_DATA_DIR


def env_flag(
    name: str,
    default: bool = False,
    *,
    truthy: Iterable[str] = ("1", "true"),
) -> bool:
    """Return whether an environment value matches an explicit truthy token."""
    value = os.environ.get(name)
    if value is None:
        return default
    accepted = {token.lower() for token in truthy}
    return value.lower() in accepted


def env_int(name: str, default: int) -> int:
    """Parse an integer environment value, falling back on empty or invalid input."""
    try:
        return int(os.environ.get(name, "") or default)
    except ValueError:
        return default


def env_float(name: str, default: float, *, minimum: Optional[float] = None) -> float:
    """Parse a float environment value with an optional lower bound."""
    try:
        value = float(os.environ.get(name, "") or default)
    except ValueError:
        return default
    return max(value, minimum) if minimum is not None else value


def env_path(name: str, default: Optional[Any] = None) -> Optional[Path]:
    """Parse an environment path and expand its user-home component."""
    value = os.environ.get(name) or default
    if value in (None, ""):
        return None
    return Path(value).expanduser()


def meshy_dcc_state_root() -> Path:
    """Return ``~/.meshy-dcc``, or ``MESHY_DCC_DATA_DIR`` when it is set.

    The data-root override keeps per-DCC state (local skills, checkpoints,
    loaded state, materialized scripts) out of the home directory for tests
    and relocated installs; unset, the documented ``~/.meshy-dcc`` stays.
    """
    return env_path(ENV_DATA_DIR) or Path("~").expanduser() / ".meshy-dcc"


__all__ = ["env_flag", "env_float", "env_int", "env_path", "meshy_dcc_state_root"]
