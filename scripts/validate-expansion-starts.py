#!/usr/bin/env python3
"""Verify two independently controlled players before expansion-agent research.

Only user-supplied pinned ROMs are read. No ROM, pixels or snapshots are written.
"""

import argparse
import hashlib
import json
from pathlib import Path

from contra_training._native import ResearchMachine


def probe(folder, target, count, x_address, y_address):
    raw = (folder / target["file"]).read_bytes()
    if hashlib.sha256(raw).hexdigest() != target["sha256"]:
        raise ValueError(f"wrong revision: {target['file']}")
    machine = ResearchMachine(raw)
    script = json.loads((Path("fixtures/compatibility") / target["gameplay_script"]).read_text())
    for segment in script["segments"][:count]:
        machine.step(segment["players"], segment["frames"])
    snapshot = bytes(machine.snapshot())
    states = {}
    for name, masks in [
        ("neutral", [0, 0]),
        ("p1-right", [128, 0]),
        ("p2-left", [0, 64]),
        ("p1-down", [32, 0]),
        ("p2-up", [0, 16]),
    ]:
        machine.restore(snapshot)
        machine.step(masks, 10)
        states[name] = bytes(machine.ram())
    neutral = states["neutral"]
    checks = [
        ("p1-right", x_address, 0, 1),
        ("p2-left", x_address, 1, -1),
        ("p1-down", y_address, 0, 1),
        ("p2-up", y_address, 1, -1),
    ]
    for name, address, player, sign in checks:
        current = states[name]
        assert (current[address + player] - neutral[address + player]) * sign > 0, name
        assert current[address + 1 - player] == neutral[address + 1 - player], (
            f"{name} affects other player"
        )
    if target["file"].startswith("Lifeforce"):
        assert list(neutral[0x70:0x72]) == [3, 3], "both ships must be controllable"
    else:
        assert neutral[0x22] != 0 and all(neutral[0x510:0x512]), "both jeeps must be present"
    return dict(
        game=target["file"],
        rom_sha256=target["sha256"],
        independent_control_cases=len(checks),
        status="passed",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rom_folder", type=Path)
    args = parser.parse_args()
    targets = json.loads(Path("fixtures/compatibility/targets.json").read_text())
    cases = {
        "Lifeforce (U).nes": (5, 0x350, 0x32F),
        "Jackal (U).nes": (14, 0x5B0, 0x570),
    }
    for target in targets:
        if target["file"] in cases:
            print(
                json.dumps(probe(args.rom_folder, target, *cases[target["file"]])),
                flush=True,
            )


if __name__ == "__main__":
    main()
