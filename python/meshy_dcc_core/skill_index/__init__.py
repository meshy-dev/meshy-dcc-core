"""Canonical Python API for optional in-process skill indexes.

The production Rust catalog and gateway continue to use
`meshy-dcc-gateway-search`. These Python indexes are application utilities for
offline lexical/vector retrieval and fusion.
"""

from meshy_dcc_core.skill_index._fusion import RrfFusionIndex
from meshy_dcc_core.skill_index._lexical import LexicalSkillIndex
from meshy_dcc_core.skill_index._protocol import SemanticSkillIndex
from meshy_dcc_core.skill_index._protocol import SkillDocument
from meshy_dcc_core.skill_index._protocol import SkillSearchHit
from meshy_dcc_core.skill_index._vector import InMemoryVectorStore
from meshy_dcc_core.skill_index._vector import VectorSkillIndex
from meshy_dcc_core.skill_index._vector import VectorStore

__all__ = [
    "InMemoryVectorStore",
    "LexicalSkillIndex",
    "RrfFusionIndex",
    "SemanticSkillIndex",
    "SkillDocument",
    "SkillSearchHit",
    "VectorSkillIndex",
    "VectorStore",
]
