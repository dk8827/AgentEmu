"""Exercise the publication boundary without modifying the checkout's index."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class PublicationScope(unittest.TestCase):
    def test_unreviewed_and_forbidden_files_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for name in json.loads((ROOT / "release-files.json").read_text()):
                dest = root / name
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / name, dest)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
            subprocess.run(["git", "add", "."], cwd=root, env=env, check=True)

            def check():
                return subprocess.run(
                    [sys.executable, "scripts/check-release.py"],
                    cwd=root,
                    env=env,
                    capture_output=True,
                    text=True,
                )

            self.assertEqual(check().returncode, 0)
            unreviewed = root / "unreviewed.txt"
            unreviewed.write_text("Not approved for distribution\n")
            subprocess.run(["git", "add", "unreviewed.txt"], cwd=root, env=env, check=True)
            result = check()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("unreviewed.txt", result.stderr)
            # Even adding a study to the allowlist cannot bypass the deny rules.
            study = root / "research/study.json"
            study.parent.mkdir()
            study.write_text("{}\n")
            names = json.loads((root / "release-files.json").read_text())
            names.extend(["unreviewed.txt", "research/study.json"])
            (root / "release-files.json").write_text(json.dumps(names))
            subprocess.run(
                ["git", "add", "-f", "research/study.json", "release-files.json"],
                cwd=root,
                env=env,
                check=True,
            )
            result = check()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Non-release material: research/study.json", result.stderr)


if __name__ == "__main__":
    unittest.main()
