"""Contract checks for the pinned MCP 2026 interop client script."""

from __future__ import annotations

from conftest import REPO_ROOT


def test_parameter_sdk_script_has_bounded_deadline_and_pinned_public_client():
    script = (REPO_ROOT / "tests/interop/mcp-2026/param-client.mjs").read_text(encoding="utf-8")
    assert "30_000" in script
    assert "process.exit(1)" in script
    assert "@modelcontextprotocol/client" in script
    assert "toolDefinition: definition" in script
    assert "Mcp-Param-Tenant" in script
