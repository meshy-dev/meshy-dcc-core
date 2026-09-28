"""Deterministic, payload-bounded verification toolset (issue #2261).

Pure-Python, standard-library-only contract that turns captures and scene
state into machine-checkable signals — image statistics, deterministic pixel
metrics, and scene-vs-spec validation — without shipping pixels to a model.

The released-product production acceptance matrix lives here too:
:mod:`meshy_dcc_core.verification.acceptance` owns the shared, versioned record
and its fail-closed evaluator, and
:mod:`meshy_dcc_core.verification.acceptance_fixtures` supplies editor-free
engine fixtures for probing it.

The behavior verification contract (core#2269) is layered on top:

* :mod:`meshy_dcc_core.verification.schemas` — versioned state-export schemas
  (``meshy-dcc/anim-curves@1``, ``meshy-dcc/rig-state@1``, ``meshy-dcc/sim-status@1``,
  ``meshy-dcc/graph-state@1``), a dependency-free structural validator, and
  animation-curve sampling helpers.
* :mod:`meshy_dcc_core.verification.assertions` — the dual-layer assertion
  library (exact structural counts, tolerance-band numerics, existence and
  resolution checks) with both fail-fast helpers and a collecting
  :class:`BehaviorVerifier` that reports a pass rate.
* :mod:`meshy_dcc_core.verification.lint` — the declaration-lint pairing rule
  that flags write verbs without a paired read-only state export.

Everything here is import-safe on Python 3.7 (Maya 2022 / Blender 2.83) and
ships in both the native and ``py37-lite`` wheels.
"""

from __future__ import annotations

from meshy_dcc_core.verification.acceptance import EVIDENCE_SOURCES
from meshy_dcc_core.verification.acceptance import LEVELS
from meshy_dcc_core.verification.acceptance import PRODUCTION_ACCEPTANCE_V1_SCHEMA_VERSION
from meshy_dcc_core.verification.acceptance import STATUSES
from meshy_dcc_core.verification.acceptance import TRANSITION_CHAIN
from meshy_dcc_core.verification.acceptance import AcceptanceEvaluation
from meshy_dcc_core.verification.acceptance import AcceptanceFinding
from meshy_dcc_core.verification.acceptance import AcceptanceValidationError
from meshy_dcc_core.verification.acceptance import build_report
from meshy_dcc_core.verification.acceptance import dumps_report
from meshy_dcc_core.verification.acceptance import evaluate_acceptance
from meshy_dcc_core.verification.acceptance import evaluate_many
from meshy_dcc_core.verification.acceptance import make_level
from meshy_dcc_core.verification.acceptance import normalize_digest
from meshy_dcc_core.verification.acceptance import production_acceptance_v1_json_schema
from meshy_dcc_core.verification.acceptance import recompute_sha256
from meshy_dcc_core.verification.acceptance import record_from_catalog_entry
from meshy_dcc_core.verification.acceptance import sanitize_evidence_link
from meshy_dcc_core.verification.acceptance import validate_acceptance_schema
from meshy_dcc_core.verification.acceptance import verify_release_sha256
from meshy_dcc_core.verification.acceptance_fixtures import MAX_GODOT_CONFIG_VERSION
from meshy_dcc_core.verification.acceptance_fixtures import MIN_GODOT_CONFIG_VERSION
from meshy_dcc_core.verification.acceptance_fixtures import MIN_TUANJIE_EDITOR_MAJOR
from meshy_dcc_core.verification.acceptance_fixtures import MIN_UNITY_EDITOR_MAJOR
from meshy_dcc_core.verification.acceptance_fixtures import MIN_UNREAL_ENGINE_MAJOR
from meshy_dcc_core.verification.acceptance_fixtures import godot_project
from meshy_dcc_core.verification.acceptance_fixtures import godot_version_probe
from meshy_dcc_core.verification.acceptance_fixtures import unity_project_version
from meshy_dcc_core.verification.acceptance_fixtures import unreal_project
from meshy_dcc_core.verification.assertions import AssertionFailure
from meshy_dcc_core.verification.assertions import BehaviorReport
from meshy_dcc_core.verification.assertions import BehaviorVerifier
from meshy_dcc_core.verification.assertions import Check
from meshy_dcc_core.verification.assertions import assert_exact
from meshy_dcc_core.verification.assertions import assert_exists
from meshy_dcc_core.verification.assertions import assert_in_band
from meshy_dcc_core.verification.assertions import assert_resolution
from meshy_dcc_core.verification.assertions import assert_state_schema
from meshy_dcc_core.verification.assertions import assert_within
from meshy_dcc_core.verification.image_stats import ImageStats
from meshy_dcc_core.verification.image_stats import ImageStatsFlags
from meshy_dcc_core.verification.image_stats import ImageStatsThresholds
from meshy_dcc_core.verification.image_stats import classify_image_stats
from meshy_dcc_core.verification.image_stats import compute_image_stats
from meshy_dcc_core.verification.image_stats import decode_ppm
from meshy_dcc_core.verification.lint import DeclarationFinding
from meshy_dcc_core.verification.lint import find_unpaired_write_verbs
from meshy_dcc_core.verification.lint import lint_tool_table
from meshy_dcc_core.verification.pixel_metrics import ahash_64
from meshy_dcc_core.verification.pixel_metrics import delta_e_2000
from meshy_dcc_core.verification.pixel_metrics import dhash_64
from meshy_dcc_core.verification.pixel_metrics import edge_density
from meshy_dcc_core.verification.pixel_metrics import hamming_distance
from meshy_dcc_core.verification.pixel_metrics import phash_64
from meshy_dcc_core.verification.pixel_metrics import silhouette_iou
from meshy_dcc_core.verification.pixel_metrics import sobel_edges
from meshy_dcc_core.verification.pixel_metrics import ssim
from meshy_dcc_core.verification.scene_spec import SceneSpecFailure
from meshy_dcc_core.verification.scene_spec import SceneSpecResult
from meshy_dcc_core.verification.scene_spec import validate_scene_vs_spec
from meshy_dcc_core.verification.schemas import SCHEMA_ANIM_CURVES
from meshy_dcc_core.verification.schemas import SCHEMA_GRAPH_STATE
from meshy_dcc_core.verification.schemas import SCHEMA_RIG_STATE
from meshy_dcc_core.verification.schemas import SCHEMA_SIM_STATUS
from meshy_dcc_core.verification.schemas import SCHEMA_VERSIONS
from meshy_dcc_core.verification.schemas import SchemaValidationError
from meshy_dcc_core.verification.schemas import sample_curve
from meshy_dcc_core.verification.schemas import schema_document
from meshy_dcc_core.verification.schemas import schema_names
from meshy_dcc_core.verification.schemas import validate_state_export
from meshy_dcc_core.verification.schemas import value_at

__all__ = [
    "EVIDENCE_SOURCES",
    "LEVELS",
    "MAX_GODOT_CONFIG_VERSION",
    "MIN_GODOT_CONFIG_VERSION",
    "MIN_TUANJIE_EDITOR_MAJOR",
    "MIN_UNITY_EDITOR_MAJOR",
    "MIN_UNREAL_ENGINE_MAJOR",
    "PRODUCTION_ACCEPTANCE_V1_SCHEMA_VERSION",
    "SCHEMA_ANIM_CURVES",
    "SCHEMA_GRAPH_STATE",
    "SCHEMA_RIG_STATE",
    "SCHEMA_SIM_STATUS",
    "SCHEMA_VERSIONS",
    "STATUSES",
    "TRANSITION_CHAIN",
    "AcceptanceEvaluation",
    "AcceptanceFinding",
    "AcceptanceValidationError",
    "AssertionFailure",
    "BehaviorReport",
    "BehaviorVerifier",
    "Check",
    "DeclarationFinding",
    "ImageStats",
    "ImageStatsFlags",
    "ImageStatsThresholds",
    "SceneSpecFailure",
    "SceneSpecResult",
    "SchemaValidationError",
    "ahash_64",
    "assert_exact",
    "assert_exists",
    "assert_in_band",
    "assert_resolution",
    "assert_state_schema",
    "assert_within",
    "build_report",
    "classify_image_stats",
    "compute_image_stats",
    "decode_ppm",
    "delta_e_2000",
    "dhash_64",
    "dumps_report",
    "edge_density",
    "evaluate_acceptance",
    "evaluate_many",
    "find_unpaired_write_verbs",
    "godot_project",
    "godot_version_probe",
    "hamming_distance",
    "lint_tool_table",
    "make_level",
    "normalize_digest",
    "phash_64",
    "production_acceptance_v1_json_schema",
    "recompute_sha256",
    "record_from_catalog_entry",
    "sample_curve",
    "sanitize_evidence_link",
    "schema_document",
    "schema_names",
    "silhouette_iou",
    "sobel_edges",
    "ssim",
    "unity_project_version",
    "unreal_project",
    "validate_acceptance_schema",
    "validate_scene_vs_spec",
    "validate_state_export",
    "value_at",
    "verify_release_sha256",
]
