"""Regression tests for the machine-readable Python support contract."""

from __future__ import annotations

from pathlib import Path

import pytest
from scripts.ci.python_support_contract import ContractError
from scripts.ci.python_support_contract import load_contract
from scripts.ci.python_support_contract import native_ci_matrix
from scripts.ci.python_support_contract import python37_test_requirements
from scripts.ci.python_support_contract import validate_contract

_REPO_ROOT = Path(__file__).resolve().parents[1]


def test_contract_rejects_calendar_expiry_or_weak_deprecation() -> None:
    contract = load_contract(_REPO_ROOT)
    contract["support"]["deprecation"]["minimum_notice_days"] = 30
    with pytest.raises(ContractError, match="at least 180 days"):
        validate_contract(contract)

    contract = load_contract(_REPO_ROOT)
    contract["support"]["deprecation"]["automatic_calendar_expiry"] = True
    with pytest.raises(ContractError, match="automatic calendar expiry"):
        validate_contract(contract)


def test_native_python37_requires_linux_and_windows() -> None:
    contract = load_contract(_REPO_ROOT)
    contract["build"]["native_py37"]["pr_matrix"] = [contract["build"]["native_py37"]["pr_matrix"][0]]
    with pytest.raises(ContractError, match="Linux and Windows"):
        validate_contract(contract)


def test_native_matrix_and_test_toolchain_are_generated_from_contract() -> None:
    contract = load_contract(_REPO_ROOT)
    matrix = native_ci_matrix(contract)
    assert [row["platform"] for row in matrix["include"]] == [
        "linux-x86_64",
        "windows-x86_64",
    ]
    assert [row["platform"] for row in matrix["include"] if row["full_suite"]] == ["linux-x86_64"]
    assert python37_test_requirements(contract) == [
        "jsonschema==4.17.3",
        "pytest==7.4.4",
        "pytest-xdist==3.5.0",
        "typing-extensions==4.7.1",
    ]


def test_native_matrix_rejects_runner_and_boolean_drift() -> None:
    contract = load_contract(_REPO_ROOT)
    contract["build"]["native_py37"]["pr_matrix"][0]["runner"] = "windows-2022"
    with pytest.raises(ContractError, match="runner or target"):
        validate_contract(contract)

    contract = load_contract(_REPO_ROOT)
    contract["build"]["native_py37"]["pr_matrix"][0]["full_suite"] = "true"
    with pytest.raises(ContractError, match="must be booleans"):
        validate_contract(contract)


def test_schema_v2_rejects_baseline_tag_and_version_gaps() -> None:
    contract = load_contract(_REPO_ROOT)
    contract["support"]["minimum_python"] = "3.8"
    contract["support"]["tested_versions"] = contract["support"]["tested_versions"][1:]
    with pytest.raises(ContractError, match=r"requires Python 3\.7"):
        validate_contract(contract)

    for profile, bad_tag, expected in (
        ("native_py37", "cp37", "cp37-cp37m"),
        ("lite_py37", "py3-any", "py3-none"),
        ("abi3", "cp38", "cp38-abi3"),
    ):
        contract = load_contract(_REPO_ROOT)
        contract["wheel_profiles"][profile]["wheel_tag"] = bad_tag
        with pytest.raises(ContractError, match=expected):
            validate_contract(contract)

    contract = load_contract(_REPO_ROOT)
    contract["support"]["tested_versions"].remove("3.8")
    with pytest.raises(ContractError, match="must be contiguous"):
        validate_contract(contract)


def test_contract_rejects_relaxed_core_or_semantic_linux_baselines() -> None:
    contract = load_contract(_REPO_ROOT)
    contract["wheel_profiles"]["native_py37"]["platforms"]["linux-x86_64"] = {
        "allowed_platform_tags": ["manylinux_2_28_x86_64"]
    }
    with pytest.raises(ContractError, match="legacy DCC platform baseline"):
        validate_contract(contract)

    contract = load_contract(_REPO_ROOT)
    contract["wheel_profiles"]["semantic_native_py37"]["platforms"]["linux-x86_64"] = {
        "allowed_platform_tags": ["manylinux_2_17_x86_64"]
    }
    with pytest.raises(ContractError, match="semantic_native_py37 linux-x86_64"):
        validate_contract(contract)
