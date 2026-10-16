"""Validate TMNT II body slots, independent controls, pause and reset against a local ROM."""

import argparse, json
from pathlib import Path
from contra_training._native import ResearchMachine, TaskEpisode

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("rom")
a = p.parse_args()
raw = Path(a.rom).read_bytes()
task = "tmnt2-scene1"
c = TaskEpisode(raw, task, 1)
initial = bytes(c.snapshot())
m = ResearchMachine(raw)
start = json.loads(c.reset(0))["info"]["state"]
assert start["gameplay"] and start["screen"] == 0
assert [p["health"] for p in start["players"]] == [60, 60]
assert [p["lives"] for p in start["players"]] == [2, 2]
for slot in [0, 1]:
    for mask, field, sign in [
        (128, "x", 1),
        (16, "ground_y", -1),
        (32, "ground_y", 1),
        (1, "y", -1),
    ]:
        m.restore(initial)
        pair = [0, 0]
        pair[slot] = mask
        m.step(pair, 12)
        before = bytes(m.snapshot())
        s = json.loads(m.observe(task))
        assert bytes(m.snapshot()) == before
        ps = s["state"]["players"]
        assert sign * (ps[slot][field] - start["players"][slot][field]) > 0
        assert ps[1 - slot][field] == start["players"][1 - slot][field]
        assert all(len(o) == 256 and all(-1 <= v <= 1 for v in o) for o in s["observations"])
    m.restore(initial)
    pair = [0, 0]
    pair[slot] = 1
    m.step(pair, 2)
    pair[slot] = 2
    m.step(pair, 2)
    s = json.loads(m.observe(task))["state"]
    assert s["players"][slot]["airborne"] and s["players"][slot]["attacking"]
    assert not s["players"][1 - slot]["attacking"]
m.restore(initial)
m.step([8, 0], 4)
assert not json.loads(m.observe(task))["gameplay"]
frozen = json.loads(m.observe(task))["state"]["players"]
m.step([128, 3], 60)
assert json.loads(m.observe(task))["state"]["players"] == frozen
for pair in [[8, 0], [0, 192], [48, 0]]:
    try:
        c.step(pair)
    except ValueError:
        pass
    else:
        raise AssertionError("invalid action accepted")
    assert bytes(c.snapshot()) == initial
assert json.loads(c.step([0, 0]))["truncated"]
c.reset(0)
assert bytes(c.snapshot()) == initial
print(
    "TMNT II: independent controls, slots 0/2, jump then attack, pause, bounded read-only observations, action rejection and deterministic reset passed"
)
