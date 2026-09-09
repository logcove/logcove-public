import hashlib
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import release


class ReleaseTests(unittest.TestCase):
    def test_tag_must_match_package_version(self):
        value = release.version()
        self.assertEqual(release.version(f"v{value}"), value)
        for tag in (value, "v999.0.0", f"v{value}-rc.1"):
            with self.assertRaises(ValueError):
                release.version(tag)

    def test_skill_archive_has_only_release_files_and_matches_sources(self):
        with tempfile.TemporaryDirectory() as temporary:
            archive = release.package_skill(release.version(), Path(temporary))
            with zipfile.ZipFile(archive) as package:
                expected = {f"logcove/{name}" for name in release.SKILL_FILES}
                expected.update(("logcove/LICENSE", "logcove/VERSION"))
                self.assertEqual(set(package.namelist()), expected)
                self.assertIsNone(package.testzip())
                for name in release.SKILL_FILES:
                    self.assertEqual(package.read(f"logcove/{name}"), (release.ROOT / "skills/logcove" / name).read_bytes())
                self.assertEqual(package.read("logcove/VERSION").decode().strip(), release.version())

    def test_archives_roundtrip_with_spaces_and_executable_mode(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "package"
            source.mkdir()
            binary = source / "logcove"
            binary.write_bytes(b"example binary")
            binary.chmod(0o755)
            (source / "read me.txt").write_text("license and instructions", encoding="utf-8")
            for extension in ("zip", "tar.gz"):
                archive = root / f"package.{extension}"
                release.archive_tree(source, archive)
                unpack = root / extension
                release.extract_archive(archive, unpack)
                self.assertEqual((unpack / "package/logcove").read_bytes(), binary.read_bytes())
                self.assertTrue((unpack / "package/read me.txt").is_file())
                if sys.platform != "win32" and extension == "tar.gz":
                    self.assertEqual((unpack / "package/logcove").stat().st_mode & 0o777, 0o755)

    def test_checksums_require_exact_complete_asset_set(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            value = release.version()
            names = [release.cli_name(value, target) for target in release.TARGETS]
            names.append(f"logcove-skills-v{value}.zip")
            names.append(f"logcove-package-managers-v{value}.zip")
            for name in names[:-1]:
                (root / name).write_bytes(name.encode())
            with self.assertRaises(ValueError):
                release.checksums(value, root)
            (root / names[-1]).write_bytes(b"skill package")
            checksums = release.checksums(value, root)
            self.assertEqual(len(checksums.read_text().splitlines()), 7)
            for line in checksums.read_text().splitlines():
                digest, name = line.split("  ")
                self.assertEqual(digest, hashlib.sha256((root / name).read_bytes()).hexdigest())
            self.assertEqual(release.checksums(value, root), checksums)
            (root / "logcove-v0.0.0-old.zip").write_bytes(b"stale")
            with self.assertRaises(ValueError):
                release.checksums(value, root)


if __name__ == "__main__":
    unittest.main()
