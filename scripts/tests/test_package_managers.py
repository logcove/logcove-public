import hashlib
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import package_managers
import release


class PackageManagerTests(unittest.TestCase):
    def test_generated_metadata_uses_actual_archive_names_and_hashes(self):
        with tempfile.TemporaryDirectory() as temporary:
            assets = Path(temporary)
            value = "0.2.0"
            for target in release.TARGETS:
                (assets / release.cli_name(value, target)).write_bytes(target.encode())
            result = package_managers.package(value, assets)
            with zipfile.ZipFile(result) as archive:
                self.assertEqual(len(archive.namelist()), 5)
                formula = archive.read("package-managers/Formula/logcove.rb").decode()
                installer = archive.read(f"package-managers/manifests/l/Logcove/Logcove/{value}/Logcove.Logcove.installer.yaml").decode()
                for target in release.TARGETS:
                    text = installer if "windows" in target else formula
                    self.assertIn(release.cli_name(value, target), text)
                    self.assertIn(hashlib.sha256(target.encode()).hexdigest(), text.lower())
                self.assertIn(f"logcove-v{value}-x86_64-pc-windows-msvc/logcove.exe", installer)
                self.assertIn('bin.install "logcove"', formula)
                self.assertNotIn("cargo", formula)

    def test_missing_archive_does_not_leave_a_metadata_package(self):
        with tempfile.TemporaryDirectory() as temporary:
            assets = Path(temporary)
            with self.assertRaises(FileNotFoundError):
                package_managers.package("0.2.0", assets)
            self.assertEqual(list(assets.iterdir()), [])

    def test_rejects_version_that_can_escape_paths_or_inject_template_content(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for value in ("../0.2.0", '0.2.0"\n', "v0.2.0"):
                with self.assertRaises(ValueError):
                    package_managers.generate(value, root, root / "output")
            self.assertFalse((root / "output").exists())


if __name__ == "__main__":
    unittest.main()
