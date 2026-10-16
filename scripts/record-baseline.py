"""Run cartridge parity suites at a clean, reproducible AgentEmu commit."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--rom-root", type=Path, required=True)
parser.add_argument("--suite", choices=["top100", "ign100"], required=True)
args = parser.parse_args()


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


if git("status", "--porcelain", "--untracked-files=no"):
    parser.error("Commit tracked source changes before recording a release baseline")
commit = git("rev-parse", "HEAD")
subprocess.run(["sh", "scripts/build-baseline.sh"], cwd=ROOT, check=True)
results_path = ROOT / f".cache/{args.suite}/results.json"
# A runner that fails before writing results must not inherit an older pass.
results_path.unlink(missing_ok=True)
result = subprocess.run(
    [
        "node",
        "scripts/test-top100.mjs",
        str(args.rom_root),
        "--suite=" + args.suite,
        "--discard-captures",
    ],
    cwd=ROOT,
)
if not results_path.exists():
    raise SystemExit("Parity runner produced no results; no baseline recorded")
rows = json.loads(results_path.read_bytes())
report = {
    "repository": "https://github.com/dk8827/AgentEmu",
    "source_commit": commit,
    "source_dirty": False,
    "artifacts": {
        name: hashlib.sha256((ROOT / path).read_bytes()).hexdigest()
        for name, path in {
            "native_sha256": "target/release/nes-cli" + (".exe" if sys.platform == "win32" else ""),
            "wasm_sha256": "web/pkg/emulator_bg.wasm",
        }.items()
    },
    "suite": args.suite,
    "suite_sha256": hashlib.sha256(
        (ROOT / f"fixtures/compatibility/{args.suite}-targets.json").read_bytes()
    ).hexdigest(),
    "scope": "Exact cartridge/script native-WASM state and PCM parity and deterministic restore; gameplay completion and independent audiovisual accuracy not assessed.",
    "passed": sum(r["status"] == "passed" for r in rows),
    "failed": sum(r["status"] == "failed" for r in rows),
    "results": rows,
}
# Reports may contain error paths; redact the user-provided root before sharing.
text = json.dumps(report, indent=2).replace(str(args.rom_root), "ROM_ROOT") + "\n"
out = ROOT / f".cache/{args.suite}/agentemu-baseline.json"
out.write_text(text)
print(out)
raise SystemExit(result.returncode)
