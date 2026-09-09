"""Validate the shipped Skill and execute its documented DuckDB examples offline."""

import contextlib
import io
import json
from pathlib import Path
import re
import tempfile
import unittest

import duckdb
import yaml

from release import ROOT, SKILL_FILES


SKILL = ROOT / "skills/logcove"
PROJECT = "prj_00000000-0000-4000-8000-000000000001"


def blocks(path, language):
    return re.findall(rf"```{language}\n(.*?)\n```", path.read_text(encoding="utf-8"), re.DOTALL)


class SkillTests(unittest.TestCase):
    def test_metadata_and_internal_references(self):
        entry = (SKILL / "SKILL.md").read_text(encoding="utf-8")
        self.assertTrue(entry.startswith("---\n"))
        metadata = yaml.safe_load(entry.split("---", 2)[1])
        self.assertEqual(metadata["name"], "logcove")
        self.assertTrue(metadata["description"].strip())
        for relative in SKILL_FILES:
            document = SKILL / relative
            for link in re.findall(r"\[[^\]]*\]\(([^)]+)\)", document.read_text(encoding="utf-8")):
                if "://" in link or link.startswith("#"):
                    continue
                target = (document.parent / link.split("#", 1)[0]).resolve()
                self.assertTrue(target.is_relative_to(SKILL), link)
                self.assertIn(target.relative_to(SKILL).as_posix(), SKILL_FILES)
                self.assertTrue(target.is_file(), link)

    def test_documented_manifest_query_and_result(self):
        reference = SKILL / "references/duckdb.md"
        setup, serialize = blocks(reference, "python")
        query, = blocks(reference, "sql")
        with tempfile.TemporaryDirectory() as temporary, contextlib.chdir(temporary):
            folder = Path("analysis/logs/pull-example")
            folder.mkdir(parents=True)
            fixture = duckdb.connect()
            self.addCleanup(fixture.close)
            fixture.sql("SELECT 'api' AS service FROM range(2)").write_parquet(str(folder / "one.parquet"))
            fixture.sql("SELECT 'worker' AS service, 500 AS status").write_parquet(str(folder / "two.parquet"))
            # A stale file outside the manifest must not affect the result.
            fixture.sql("SELECT 'stale' AS service FROM range(10)").write_parquet(str(folder / "stale.parquet"))
            manifest = {"project_id": PROJECT, "files": [{"path": "one.parquet"}, {"path": "two.parquet"}]}
            (folder / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            Path("query.sql").write_text(query, encoding="utf-8")
            namespace = {}
            with contextlib.redirect_stdout(io.StringIO()):
                exec(compile(setup, str(reference), "exec"), namespace)
            self.addCleanup(namespace["db"].close)
            exec(compile(serialize, str(reference), "exec"), namespace)
            result = json.loads(Path("result.json").read_text(encoding="utf-8"))
            self.assertEqual(result["data"], [{"service": "api", "requests": 2}, {"service": "worker", "requests": 1}])
            self.assertEqual(set(result), {"computed_at", "data"})
            spec = json.loads(blocks(SKILL / "references/charts.md", "json")[0])
            self.assertEqual(spec["data"], {"name": "result"})
            for encoding in spec["encoding"].values():
                for field in encoding if isinstance(encoding, list) else [encoding]:
                    self.assertIn(field["field"], result["data"][0])

            # Exercise the reference's limits and distinguish an empty result from no files.
            for sql, message in [
                ("SELECT i FROM range(10001) t(i)", "10000 rows"),
                ("SELECT 1 AS value, 2 AS value", "unique SQL aliases"),
                ("SELECT repeat('x', 5242880) AS value", "5 MiB"),
                ("SELECT 'NaN'::DOUBLE AS value", "Out of range float"),
            ]:
                Path("query.sql").write_text(sql, encoding="utf-8")
                with self.assertRaisesRegex(ValueError, message):
                    exec(compile(serialize, str(reference), "exec"), namespace)
            Path("query.sql").write_text(f'SELECT service FROM "{PROJECT}" WHERE false', encoding="utf-8")
            exec(compile(serialize, str(reference), "exec"), namespace)
            self.assertEqual(json.loads(Path("result.json").read_text())["data"], [])
            manifest["files"] = []
            (folder / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "No files"):
                exec(compile(setup, str(reference), "exec"), {})


if __name__ == "__main__":
    unittest.main(verbosity=2)
