"""Tests for utility functions: filesystem, type wrappers, constants."""

# Import future modules
from __future__ import annotations

# Import built-in modules
from pathlib import Path

# Import third-party modules
import pytest

# Import local modules
import meshy_dcc_core


class TestConstants:
    @pytest.mark.parametrize(
        ("attr", "expected"),
        [
            ("APP_NAME", "meshy-dcc"),
            ("APP_AUTHOR", "meshy-dcc"),
            ("DEFAULT_DCC", "python"),
            ("SKILL_METADATA_FILE", "SKILL.md"),
            ("SKILL_SCRIPTS_DIR", "scripts"),
            ("ENV_SKILL_PATHS", "MESHY_DCC_SKILL_PATHS"),
            ("ENV_DISABLE_DEFAULT_SKILL_PATHS", "MESHY_DCC_DISABLE_DEFAULT_SKILL_PATHS"),
            ("ENV_LOG_LEVEL", "MESHY_DCC_LOG_LEVEL"),
            ("DEFAULT_LOG_LEVEL", "DEBUG"),
            ("DEFAULT_MIME_TYPE", "text/plain"),
            ("DEFAULT_VERSION", "1.0.0"),
            ("SKILL_METADATA_DIR", "metadata"),
        ],
    )
    def test_constant_values(self, attr: str, expected: str) -> None:
        assert getattr(meshy_dcc_core, attr) == expected


class TestFilesystem:
    def test_get_config_dir(self) -> None:
        path = meshy_dcc_core.get_config_dir()
        assert "meshy-dcc" in path

    def test_get_data_dir(self) -> None:
        path = meshy_dcc_core.get_data_dir()
        assert "meshy-dcc" in path

    def test_get_log_dir(self) -> None:
        path = meshy_dcc_core.get_log_dir()
        assert "meshy-dcc" in path
        assert "log" in path

    @pytest.mark.parametrize("dir_type", ["config", "data", "cache", "log", "state"])
    def test_get_platform_dir(self, dir_type: str) -> None:
        path = meshy_dcc_core.get_platform_dir(dir_type)
        assert "meshy-dcc" in path

    def test_get_platform_dir_documents(self) -> None:
        try:
            path = meshy_dcc_core.get_platform_dir("documents")
        except (ValueError, OSError):
            pytest.skip("documents directory not available on this platform")
        assert "meshy-dcc" in path

    def test_get_platform_dir_invalid(self) -> None:
        with pytest.raises(ValueError):
            meshy_dcc_core.get_platform_dir("invalid_type")

    def test_get_actions_dir(self) -> None:
        path = meshy_dcc_core.get_tools_dir("maya")
        assert "maya" in path
        assert "actions" in path

    def test_get_actions_dir_different_dcc(self) -> None:
        path = meshy_dcc_core.get_tools_dir("blender")
        assert "blender" in path

    def test_get_skills_dir_no_dcc(self) -> None:
        path = meshy_dcc_core.get_skills_dir()
        assert isinstance(path, str)
        assert "skills" in path

    def test_get_skills_dir_with_dcc(self) -> None:
        path = meshy_dcc_core.get_skills_dir("maya")
        assert "maya" in path.lower()
        assert "skills" in path

    def test_get_skill_paths_from_env_empty(self) -> None:
        paths = meshy_dcc_core.get_skill_paths_from_env()
        assert isinstance(paths, list)

    def test_get_skill_paths_from_env_with_var(self, monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
        monkeypatch.setenv("MESHY_DCC_SKILL_PATHS", str(tmp_path))
        paths = meshy_dcc_core.get_skill_paths_from_env()
        assert str(tmp_path) in paths


class TestTypeWrappers:
    def test_boolean_wrapper(self) -> None:
        w = meshy_dcc_core.BooleanWrapper(True)
        assert w.value is True
        assert bool(w) is True

    def test_boolean_wrapper_false(self) -> None:
        w = meshy_dcc_core.BooleanWrapper(False)
        assert w.value is False
        assert bool(w) is False

    def test_boolean_wrapper_repr(self) -> None:
        assert "True" in repr(meshy_dcc_core.BooleanWrapper(True))
        assert "False" in repr(meshy_dcc_core.BooleanWrapper(False))

    def test_boolean_wrapper_eq(self) -> None:
        w = meshy_dcc_core.BooleanWrapper(True)
        assert w == True
        assert w != False
        assert w != "not a bool"

    def test_int_wrapper(self) -> None:
        w = meshy_dcc_core.IntWrapper(42)
        assert w.value == 42
        assert int(w) == 42

    def test_int_wrapper_index(self) -> None:
        w = meshy_dcc_core.IntWrapper(3)
        # __index__ makes it usable as list index
        lst = [10, 20, 30, 40]
        assert lst[w] == 40  # index 3

    def test_int_wrapper_negative(self) -> None:
        w = meshy_dcc_core.IntWrapper(-5)
        assert w.value == -5
        assert int(w) == -5

    def test_int_wrapper_repr(self) -> None:
        assert "42" in repr(meshy_dcc_core.IntWrapper(42))

    def test_float_wrapper(self) -> None:
        w = meshy_dcc_core.FloatWrapper(3.14)
        assert w.value == 3.14
        assert float(w) == 3.14

    def test_float_wrapper_repr(self) -> None:
        assert "3.14" in repr(meshy_dcc_core.FloatWrapper(3.14))

    def test_string_wrapper(self) -> None:
        w = meshy_dcc_core.StringWrapper("hello")
        assert w.value == "hello"
        assert str(w) == "hello"

    def test_string_wrapper_empty(self) -> None:
        w = meshy_dcc_core.StringWrapper("")
        assert w.value == ""
        assert str(w) == ""

    def test_string_wrapper_repr(self) -> None:
        assert "hello" in repr(meshy_dcc_core.StringWrapper("hello"))

    # unwrap_value
    def test_unwrap_value_bool(self) -> None:
        v = meshy_dcc_core.unwrap_value(meshy_dcc_core.BooleanWrapper(True))
        assert v is True

    def test_unwrap_value_int(self) -> None:
        v = meshy_dcc_core.unwrap_value(meshy_dcc_core.IntWrapper(99))
        assert v == 99

    def test_unwrap_value_float(self) -> None:
        v = meshy_dcc_core.unwrap_value(meshy_dcc_core.FloatWrapper(2.718))
        assert v == 2.718

    def test_unwrap_value_string(self) -> None:
        v = meshy_dcc_core.unwrap_value(meshy_dcc_core.StringWrapper("hi"))
        assert v == "hi"

    def test_unwrap_value_passthrough_str(self) -> None:
        v = meshy_dcc_core.unwrap_value("plain")
        assert v == "plain"

    def test_unwrap_value_passthrough_int(self) -> None:
        v = meshy_dcc_core.unwrap_value(42)
        assert v == 42

    def test_unwrap_value_passthrough_none(self) -> None:
        v = meshy_dcc_core.unwrap_value(None)
        assert v is None

    # unwrap_parameters
    def test_unwrap_parameters(self) -> None:
        result = meshy_dcc_core.unwrap_parameters(
            {
                "bool_key": meshy_dcc_core.BooleanWrapper(True),
                "int_key": meshy_dcc_core.IntWrapper(10),
                "float_key": meshy_dcc_core.FloatWrapper(1.5),
                "str_key": meshy_dcc_core.StringWrapper("val"),
                "plain": "hello",
            }
        )
        assert result["bool_key"] is True
        assert result["int_key"] == 10
        assert result["float_key"] == 1.5
        assert result["str_key"] == "val"
        assert result["plain"] == "hello"

    def test_unwrap_parameters_empty(self) -> None:
        result = meshy_dcc_core.unwrap_parameters({})
        assert result == {}

    # wrap_value
    def test_wrap_value_bool(self) -> None:
        w = meshy_dcc_core.wrap_value(True)
        assert isinstance(w, meshy_dcc_core.BooleanWrapper)
        assert w.value is True

    def test_wrap_value_int(self) -> None:
        w = meshy_dcc_core.wrap_value(42)
        assert isinstance(w, meshy_dcc_core.IntWrapper)
        assert w.value == 42

    def test_wrap_value_float(self) -> None:
        w = meshy_dcc_core.wrap_value(3.14)
        assert isinstance(w, meshy_dcc_core.FloatWrapper)
        assert w.value == 3.14

    def test_wrap_value_string(self) -> None:
        w = meshy_dcc_core.wrap_value("hello")
        assert isinstance(w, meshy_dcc_core.StringWrapper)
        assert w.value == "hello"

    def test_wrap_value_passthrough(self) -> None:
        lst = [1, 2, 3]
        v = meshy_dcc_core.wrap_value(lst)
        assert v == [1, 2, 3]  # unsupported types pass through


class TestVersion:
    def test_version_exists(self) -> None:
        assert hasattr(meshy_dcc_core, "__version__")
        assert isinstance(meshy_dcc_core.__version__, str)
        assert meshy_dcc_core.__version__ != ""

    def test_version_fallback_on_exception(self) -> None:
        import contextlib
        import importlib
        import sys
        import types

        # Create a fake _core module without __version__
        fake_core = types.ModuleType("meshy_dcc_core._core")
        # Copy all attributes except __version__
        for attr in dir(meshy_dcc_core._core):
            if attr == "__version__":
                continue
            with contextlib.suppress(AttributeError, TypeError):
                setattr(fake_core, attr, getattr(meshy_dcc_core._core, attr))

        original_core = sys.modules["meshy_dcc_core._core"]
        original_pkg = sys.modules["meshy_dcc_core"]
        try:
            sys.modules["meshy_dcc_core._core"] = fake_core
            # Remove cached meshy_dcc_core to force fresh import with fake _core
            del sys.modules["meshy_dcc_core"]
            import meshy_dcc_core as reimported

            assert reimported.__version__ == "0.0.0-dev"
        finally:
            # Restore original modules
            sys.modules["meshy_dcc_core._core"] = original_core
            sys.modules["meshy_dcc_core"] = original_pkg
            importlib.reload(meshy_dcc_core)

    def test_all_exports(self) -> None:
        assert hasattr(meshy_dcc_core, "__all__")
        for name in meshy_dcc_core.__all__:
            assert hasattr(meshy_dcc_core, name), f"Missing export: {name}"

    def test_export_metadata_matches_facade(self) -> None:
        from meshy_dcc_core._exports import _EXPERIMENTAL_LAZY
        from meshy_dcc_core._exports import _LAZY
        from meshy_dcc_core._exports import _OPTIONAL
        from meshy_dcc_core._exports import _STABLE_LAZY
        from meshy_dcc_core._exports import PUBLIC_EXPORTS
        from meshy_dcc_core._exports import _public_export_sort_key

        assert meshy_dcc_core.__all__ == PUBLIC_EXPORTS
        assert sorted(PUBLIC_EXPORTS, key=_public_export_sort_key) == PUBLIC_EXPORTS
        assert set(PUBLIC_EXPORTS) == set(_STABLE_LAZY) | {"__author__", "__version__"}
        assert set(_LAZY) == set(_STABLE_LAZY) | set(_EXPERIMENTAL_LAZY)
        assert set(_STABLE_LAZY).isdisjoint(_EXPERIMENTAL_LAZY)
        assert len(PUBLIC_EXPORTS) == len(set(PUBLIC_EXPORTS))
        assert set(_OPTIONAL).issubset(_LAZY)

    def test_core_module_accessible(self) -> None:
        assert hasattr(meshy_dcc_core, "_core")

    def test_author(self) -> None:
        assert meshy_dcc_core._core.__author__ == "Meshy <support@meshy.ai>"
