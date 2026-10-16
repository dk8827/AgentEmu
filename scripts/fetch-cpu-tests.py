"""Fetch pinned MIT-licensed CPU traces; retain full corpus locally and small CI samples."""

import concurrent.futures
import hashlib
import json
from pathlib import Path
import re
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
REV = "2f6980a2d95757486c7bee24355c360e40e2a224"
BASE = f"https://raw.githubusercontent.com/SingleStepTests/65x02/{REV}"
CACHE = ROOT / ".cache" / "nes6502"
FIXTURES = ROOT / "fixtures" / "cpu"
CACHE.mkdir(parents=True, exist_ok=True)
FIXTURES.mkdir(parents=True, exist_ok=True)
opcodes = re.findall(r"0x([0-9a-f]{2}) =>", (ROOT / "crates/nes-core/src/opcodes.rs").read_text())


def fetch(opcode):
    path = CACHE / f"{opcode}.json"
    if not path.exists():
        with urllib.request.urlopen(f"{BASE}/nes6502/v1/{opcode}.json", timeout=120) as response:
            data = response.read()
        path.write_bytes(data)
    data = path.read_bytes()
    tests = json.loads(data)
    # Fixed samples spread across each file; the full 10,000 cases remain cached.
    samples = [tests[i] for i in range(0, len(tests), 625)]
    (FIXTURES / f"{opcode}.json").write_text(json.dumps(samples, separators=(",", ":")) + "\n")
    return {
        "opcode": opcode,
        "sha256": hashlib.sha256(data).hexdigest(),
        "cases": len(tests),
        "sampled": len(samples),
    }


with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
    results = list(pool.map(fetch, opcodes))
with urllib.request.urlopen(f"{BASE}/LICENSE", timeout=120) as response:
    (FIXTURES / "LICENSE").write_bytes(response.read())
(FIXTURES / "provenance.json").write_text(
    json.dumps(
        {
            "repository": "https://github.com/SingleStepTests/65x02",
            "revision": REV,
            "directory": "nes6502/v1",
            "files": results,
        },
        indent=2,
    )
    + "\n"
)
print(
    f"Fetched {sum(r['cases'] for r in results):,} cases; saved {sum(r['sampled'] for r in results):,} CI samples."
)
