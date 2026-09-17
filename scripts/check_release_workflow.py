"""Exercise the real publishing shell block with a fake gh command."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import yaml

from release import ROOT


MOCK_GH = r'''
gh() {
  printf '%s\n' "$*" >> "$MOCK_LOG"
  case "$1 $2" in
    'release view')
      case "$MOCK_STATE" in
        absent) return 1 ;;
        draft) printf 'true\n' ;;
        public) printf 'false\n' ;;
      esac ;;
    'release upload') if [[ "$MOCK_FAIL_UPLOAD" == 1 ]]; then return 1; fi ;;
  esac
  return 0
}
'''


class PublishTests(unittest.TestCase):
    def test_publication_states(self):
        workflow = yaml.safe_load((ROOT / ".github/workflows/release.yml").read_text())
        script = workflow["jobs"]["publish"]["steps"][-1]["run"]
        cases = (
            ("absent", "v0.1.0", False),
            ("draft", "v0.1.0", False),
            ("public", "v0.1.0", False),
            ("absent", "v0.1.0-rc.1", False),
            ("absent", "v0.1.0", True),
        )
        for state, tag, failure in cases:
            with self.subTest(state=state, tag=tag, failure=failure), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                (root / "dist").mkdir()
                (root / "dist/fixture.zip").write_text("fixture")
                notes = root / f"docs/release-notes/{tag}.md"
                notes.parent.mkdir(parents=True)
                notes.write_text("Release notes\n\nUpgrade instructions.\n")
                env = dict(
                    os.environ, MOCK_STATE=state, MOCK_FAIL_UPLOAD=str(int(failure)),
                    MOCK_LOG=str(root / "calls"), RELEASE_TAG=tag,
                )
                result = subprocess.run(
                    ["bash", "-c", MOCK_GH + script], cwd=root, env=env,
                    capture_output=True, text=True,
                )
                calls = (root / "calls").read_text().splitlines()
                published = any(call.startswith("release edit") for call in calls)
                self.assertEqual(published, state != "public" and not failure, calls)
                self.assertEqual(result.returncode == 0, published, result.stderr)
                if state == "public":
                    self.assertEqual(len(calls), 1)
                if state == "draft":
                    self.assertFalse(any(call.startswith("release create") for call in calls))
                if published:
                    self.assertTrue(calls[-2].startswith("release upload"), calls)
                    self.assertIn(f'--prerelease={str("-" in tag).lower()}', calls[-1])
                    self.assertIn(f"--notes-file docs/release-notes/{tag}.md", calls[-1])

    def test_missing_release_notes_prevent_remote_changes(self):
        workflow = yaml.safe_load((ROOT / ".github/workflows/release.yml").read_text())
        script = workflow["jobs"]["publish"]["steps"][-1]["run"]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            env = dict(os.environ, MOCK_STATE="absent", MOCK_FAIL_UPLOAD="0",
                       MOCK_LOG=str(root / "calls"), RELEASE_TAG="v0.3.0")
            result = subprocess.run(["bash", "-c", MOCK_GH + script], cwd=root, env=env,
                                    capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((root / "calls").exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
