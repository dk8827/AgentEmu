#!/usr/bin/env python3
"""Run public hardware test ROMs from a pinned, ignored local checkout."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CHECKOUT = ROOT / ".cache/nes-test-roms"
REVISION = "95d8f621ae55cee0d09b91519a8989ae0e64753b"
REPOSITORY = "https://github.com/christopherpow/nes-test-roms.git"
SUITES = {
    "apu": "apu_test/rom_singles",
    "sprite-hit": "sprite_hit_tests_2005.10.05",
    "sprite-overflow": "sprite_overflow_tests",
    "vblank": "ppu_vbl_nmi/rom_singles",
    "interrupts": "cpu_interrupts_v2/rom_singles",
    "mmc3": "mmc3_test_2/rom_singles",
    "ppu-open-bus": "ppu_open_bus",
    "oam-read": "oam_read",
    "dma-overlap": "sprdma_and_dmc_dma",
    "dma-registers": "dmc_dma_during_read4",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--suite", choices=SUITES, action="append")
    parser.add_argument("--report", type=Path, default=ROOT / ".cache/conformance.json")
    args = parser.parse_args()
    if not CHECKOUT.exists() and args.fetch:
        CHECKOUT.parent.mkdir(exist_ok=True)
        subprocess.run(["git", "clone", REPOSITORY, str(CHECKOUT)], check=True)
        subprocess.run(["git", "-C", str(CHECKOUT), "checkout", "--detach", REVISION], check=True)
    if not CHECKOUT.exists():
        parser.error("test ROM checkout missing; run with --fetch")
    revision = subprocess.check_output(
        ["git", "-C", str(CHECKOUT), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != REVISION:
        parser.error(f"expected test source {REVISION}; found {revision}")
    subprocess.run(
        ["cargo", "build", "--locked", "--release", "-p", "nes-cli"], cwd=ROOT, check=True
    )
    executable = ROOT / "target/release" / ("nes-cli.exe" if sys.platform == "win32" else "nes-cli")
    paths = []
    for suite in args.suite or SUITES:
        found = sorted((CHECKOUT / SUITES[suite]).glob("*.nes"))
        if suite == "dma-registers":
            # The other two binaries print phase-dependent CRCs, not a pass/fail result.
            found = [
                p
                for p in found
                if p.name in {"dma_4016_read.nes", "dma_2007_write.nes", "read_write_2007.nes"}
            ]
        if not found:
            parser.error(f"no ROMs found for {suite}")
        paths.extend(found)

    def run(path):
        command = [str(executable), "test-rom", str(path), "1200"]
        if path.name == "6-MMC3_alt.nes":
            command.append("--mmc3-alt")
        result = subprocess.run(command, capture_output=True, text=True)
        return {
            "test": str(path.relative_to(CHECKOUT)),
            "passed": result.returncode == 0,
            "output": (result.stdout + result.stderr).strip(),
        }

    with ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(run, paths))
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(
        json.dumps({"source": REPOSITORY, "revision": revision, "results": results}, indent=2)
        + "\n"
    )
    for result in results:
        print(("PASS " if result["passed"] else "FAIL ") + result["test"])
        if not result["passed"]:
            print(result["output"])
    print(f"{sum(r['passed'] for r in results)}/{len(results)} passed; report: {args.report}")
    return 0 if all(result["passed"] for result in results) else 1


if __name__ == "__main__":
    sys.exit(main())
