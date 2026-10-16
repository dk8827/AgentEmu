"""Local Bubble Bobble revision/semantic checks; no ROM or captures are committed."""

import argparse
import json
from pathlib import Path
from contra_training._native import ResearchMachine, TaskEpisode


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("rom")
    a = p.parse_args()
    raw = Path(a.rom).read_bytes()
    task = "bubblebobble-round1"
    c = TaskEpisode(raw, task, 1)
    first = json.loads(c.reset(0))
    initial = bytes(c.snapshot())
    assert first["info"]["state"]["round"] == 1
    assert [p["lives"] for p in first["info"]["state"]["players"]] == [3, 3]
    assert len(first["info"]["state"]["entities"]) == 3
    m = ResearchMachine(raw)
    positions = []
    for masks in [[0, 0], [128, 0], [0, 64], [1, 0], [0, 1]]:
        m.restore(initial)
        m.step(masks, 15)
        before = bytes(m.snapshot())
        s = json.loads(m.observe(task))
        assert bytes(m.snapshot()) == before
        assert all(len(o) == 256 and all(-1 <= v <= 1 for v in o) for o in s["observations"])
        positions.append(s["state"]["players"])
    assert (
        positions[1][0]["x"] > positions[0][0]["x"] and positions[1][1]["x"] == positions[0][1]["x"]
    )
    assert (
        positions[2][1]["x"] < positions[0][1]["x"] and positions[2][0]["x"] == positions[0][0]["x"]
    )
    assert (
        positions[3][0]["y"] < positions[0][0]["y"] and positions[3][1]["y"] == positions[0][1]["y"]
    )
    assert (
        positions[4][1]["y"] < positions[0][1]["y"] and positions[4][0]["y"] == positions[0][0]["y"]
    )
    for masks in [[8, 0], [0, 192], [48, 0]]:
        try:
            c.step(masks)
        except ValueError:
            pass
        else:
            raise AssertionError("invalid training action accepted")
        assert bytes(c.snapshot()) == initial
    assert json.loads(c.step([0, 0]))["truncated"]
    c.reset(0)
    assert bytes(c.snapshot()) == initial
    print(
        "Bubble Bobble: both players move/jump independently; lives, entities, reset, bounds and read-only observations passed"
    )


if __name__ == "__main__":
    main()
