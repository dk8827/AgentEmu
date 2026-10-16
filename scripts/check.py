"""Run named development checks from any working directory."""

import argparse
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("suite", choices=["core", "browser-unit", "browser", "training", "release"])
args = parser.parse_args()
os.chdir(ROOT)


def run(*command, env=None):
    print("+ " + " ".join(command), flush=True)
    subprocess.run(command, check=True, env=env)


browser_tests = [
    "wasm",
    "player",
    "contra-wasm",
    "replay",
    "input-state",
    "agent-scope",
    "frame-clock",
    "shared-control",
    "runtime-audio",
    "compatibility-evidence",
]
if args.suite == "core":
    run("cargo", "fmt", "--all", "--check")
    run("cargo", "clippy", "--locked", "--workspace", "--all-targets", "--", "-D", "warnings")
    run("cargo", "test", "--locked", "--workspace")
elif args.suite == "browser-unit":
    run("node", "--test", *(f"scripts/test-{name}.mjs" for name in browser_tests))
elif args.suite == "browser":
    run("sh", "scripts/build-web.sh")
    run("npm", "--prefix", "web", "ci")
    run(sys.executable, "scripts/check.py", "browser-unit")
    run(sys.executable, "scripts/package-web.py")
    run("node", "scripts/test-rust-ui.mjs", env={**os.environ, "AGENTEMU_PACKAGED": "1"})
elif args.suite == "training":
    run(sys.executable, "-m", "unittest", "discover", "-s", "training/tests")
else:
    run(sys.executable, "scripts/sync-integrations.py", "--check")
    run(sys.executable, "scripts/sync-catalog.py", "--check")
    run(sys.executable, "scripts/test_model_evidence.py")
    run(sys.executable, "scripts/test_release_scope.py")
    run(sys.executable, "scripts/check-release.py")
