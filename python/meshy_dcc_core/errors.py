"""Shared Python exception hierarchy for meshy-dcc-core."""

from __future__ import annotations


class MeshyDccError(Exception):
    """Base class for public meshy-dcc-core Python exceptions."""


__all__ = ["MeshyDccError"]
