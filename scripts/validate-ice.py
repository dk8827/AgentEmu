"""Local US Ice Climber structured-state and controller checks."""

import argparse
import json
from pathlib import Path
from contra_training._native import ResearchMachine, TaskEpisode

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("rom")
a = p.parse_args()
raw = Path(a.rom).read_bytes()
task = "iceclimber-mountain1"
c = TaskEpisode(raw, task, 1)
initial = bytes(c.snapshot())
first = json.loads(c.reset(0))["info"]["state"]
assert first["mountain"] == first["completed"] == 0
assert first["phase"] == 1 and first["gameplay"]
assert [p["lives"] for p in first["players"]] == [3, 3]
assert [p["floor"] for p in first["players"]] == [1, 1]
m = ResearchMachine(raw)
positions = []
for masks in [[0, 0], [128, 0], [0, 64], [1, 0], [0, 1]]:
    m.restore(initial)
    # Release the reset menu buttons before pressing jump.
    m.step([0, 0], 3)
    m.step(masks, 12)
    before = bytes(m.snapshot())
    s = json.loads(m.observe(task))
    assert bytes(m.snapshot()) == before
    assert all(len(o) == 256 and all(-1 <= v <= 1 for v in o) for o in s["observations"])
    positions.append(s["state"]["players"])
assert positions[1][0]["x"] > positions[0][0]["x"] and positions[1][1]["x"] == positions[0][1]["x"]
assert positions[2][1]["x"] < positions[0][1]["x"] and positions[2][0]["x"] == positions[0][0]["x"]
assert positions[3][0]["y"] < positions[0][0]["y"] and positions[3][1]["y"] == positions[0][1]["y"]
assert positions[4][1]["y"] < positions[0][1]["y"] and positions[4][0]["y"] == positions[0][0]["y"]
assert positions[3][0]["jump_held"] and not positions[3][1]["jump_held"]
m.restore(initial)
for t in range(121):
    m.step([1 if t % 30 < 2 else 0, 0], 1)
f = json.loads(m.observe(task))["state"]
assert any(v & 128 for v in f["floor_tiles"][1])
assert all(v == 1 for v in first["floor_tiles"][1])
for masks in [[8, 0], [0, 192], [48, 0]]:
    try:
        c.step(masks)
    except ValueError:
        pass
    else:
        raise AssertionError("invalid input accepted")
    assert bytes(c.snapshot()) == initial
assert json.loads(c.step([0, 0]))["truncated"]
c.reset(0)
assert bytes(c.snapshot()) == initial
print(
    "Ice Climber: separate movement/jumps, held buttons, broken terrain, reset, action rejection and read-only bounded observations passed"
)
