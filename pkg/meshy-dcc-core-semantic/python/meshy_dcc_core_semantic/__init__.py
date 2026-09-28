"""Native Rust semantic embeddings for ``meshy-dcc-core``.

This companion package is installed by ``pip install 'meshy-dcc-core[semantic]'``
or directly via ``pip install meshy-dcc-core-semantic``. It ships a single
native extension :mod:`meshy_dcc_core_semantic._native` (built by maturin
from ``crates/meshy-dcc-semantic``) that wraps `fastembed-rs
<https://github.com/Anush008/fastembed-rs>`_ for ONNX-backed sentence
embeddings.

Adapters do **not** import from this module directly; the public surface
is :class:`meshy_dcc_core.OnnxEmbedder`, which prefers this Rust backend
when available and falls back to the Python ``fastembed`` package
otherwise. See :mod:`meshy_dcc_core.vector_embedder` for the full fallback
chain.

Re-exports:

* :data:`native` — the compiled ``_native`` submodule with
  ``NativeEmbedder``, ``DEFAULT_MODEL``, and ``SUPPORTED_MODELS``.
"""

from __future__ import annotations

from meshy_dcc_core_semantic import _native as native
from meshy_dcc_core_semantic._native import NativeEmbedder

# Compatibility alias for earlier versions that exposed the Python wrapper here
OnnxEmbedder = NativeEmbedder

__all__ = ["NativeEmbedder", "OnnxEmbedder", "native"]
__version__ = native.__version__
