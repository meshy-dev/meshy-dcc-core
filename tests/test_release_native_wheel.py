"""The release gate must load the native extension from the produced wheel."""

import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import zipfile


spec = importlib.util.spec_from_file_location(
    "scrub_artifacts", Path(__file__).resolve().parents[1] / "scripts/release/scrub_artifacts.py"
)
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class NativeWheelGateTests(unittest.TestCase):
    def wheel(self, root, filename, contents):
        path = root / "meshy_dcc_core-0.0.0-py3-none-any.whl"
        info = "meshy_dcc_core-0.0.0.dist-info"
        files = {
            "meshy_dcc_core/__init__.py": "",
            "meshy_dcc_core/" + filename: contents,
            info + "/METADATA": "Metadata-Version: 2.1\nName: meshy-dcc-core\nVersion: 0.0.0\n",
            info + "/WHEEL": "Wheel-Version: 1.0\nRoot-Is-Purelib: true\nTag: py3-none-any\n",
        }
        files[info + "/RECORD"] = "".join(name + ",,\n" for name in files) + info + "/RECORD,,\n"
        with zipfile.ZipFile(path, "w") as archive:
            for name, value in files.items():
                archive.writestr(name, value)
        return path

    def test_python_fallback_cannot_pass_the_native_gate(self):
        with tempfile.TemporaryDirectory() as scratch:
            wheel = self.wheel(Path(scratch), "_core.py", "value = 'fallback'\n")
            with self.assertRaises(subprocess.CalledProcessError):
                release.verify_native_wheel(wheel)

    def test_invalid_native_library_cannot_pass_by_package_import_alone(self):
        with tempfile.TemporaryDirectory() as scratch:
            suffix = ".pyd" if os.name == "nt" else ".so"
            wheel = self.wheel(Path(scratch), "_core" + suffix, b"invalid native library")
            with self.assertRaises(subprocess.CalledProcessError):
                release.verify_native_wheel(wheel)


if __name__ == "__main__":
    unittest.main()
