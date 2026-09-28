"""Tests for ToolsListStubPolicy env resolution (issues #174 / #238)."""

from __future__ import annotations

import os

import pytest

from meshy_dcc_core import McpHttpConfig
from meshy_dcc_core import ToolsListStubPolicy
from meshy_dcc_core import apply_tools_list_stub_policy
from meshy_dcc_core import dcc_exclude_stubs_env_name
from meshy_dcc_core import resolve_tools_list_stub_policy


def test_dcc_exclude_stubs_env_name_normalises_slug():
    assert dcc_exclude_stubs_env_name("maya") == "MESHY_DCC_MAYA_EXCLUDE_STUBS_FROM_TOOLS_LIST"
    assert dcc_exclude_stubs_env_name("3ds-max") == "MESHY_DCC_3DS_MAX_EXCLUDE_STUBS_FROM_TOOLS_LIST"


def test_resolve_per_dcc_env_wins_over_global(monkeypatch):
    monkeypatch.delenv("MESHY_DCC_MAYA_EXCLUDE_STUBS_FROM_TOOLS_LIST", raising=False)
    monkeypatch.setenv("MESHY_DCC_EXCLUDE_STUBS_FROM_TOOLS_LIST", "0")
    monkeypatch.setenv("MESHY_DCC_BLENDER_EXCLUDE_STUBS_FROM_TOOLS_LIST", "1")
    policy = resolve_tools_list_stub_policy("blender")
    assert policy.exclude_skill_stubs is True
    assert policy.exclude_group_stubs is True


def test_resolve_maya_per_dcc_env(monkeypatch):
    monkeypatch.delenv("MESHY_DCC_MAYA_EXCLUDE_STUBS_FROM_TOOLS_LIST", raising=False)
    monkeypatch.delenv("MESHY_DCC_EXCLUDE_STUBS_FROM_TOOLS_LIST", raising=False)
    monkeypatch.setenv("MESHY_DCC_MAYA_EXCLUDE_STUBS_FROM_TOOLS_LIST", "yes")
    policy = resolve_tools_list_stub_policy("maya")
    assert policy == ToolsListStubPolicy.exclude_all_progressive_stubs()


def test_apply_writes_mcp_http_config():
    cfg = McpHttpConfig(port=0)
    policy = apply_tools_list_stub_policy(
        cfg,
        "houdini",
        explicit=ToolsListStubPolicy(exclude_skill_stubs=True, exclude_group_stubs=False),
    )
    assert policy.exclude_skill_stubs is True
    assert policy.exclude_group_stubs is False
    assert cfg.exclude_skill_stubs_from_tools_list is True
    assert cfg.exclude_group_stubs_from_tools_list is False


def test_explicit_overrides_env(monkeypatch):
    monkeypatch.setenv("MESHY_DCC_EXCLUDE_STUBS_FROM_TOOLS_LIST", "1")
    policy = resolve_tools_list_stub_policy(
        "maya",
        explicit=ToolsListStubPolicy(),
    )
    assert policy.exclude_skill_stubs is False


def test_dcc_name_required_for_env_helper():
    with pytest.raises(ValueError):
        dcc_exclude_stubs_env_name("   ")
