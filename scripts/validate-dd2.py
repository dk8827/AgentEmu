"""Check Normal co-op DD2 reset, both ports, pause, read-only fields and actions."""

import argparse, json
from pathlib import Path
from contra_training._native import TaskEpisode, ResearchMachine

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("rom")
a = p.parse_args()
raw = Path(a.rom).read_bytes()
task = "doubledragon2-mission1"
c = TaskEpisode(raw, task, 1)
initial = bytes(c.snapshot())
s = json.loads(c.reset(0))["info"]["state"]
assert s["gameplay"] and s["difficulty"] == 1 and not s["friendly_fire"]
assert [p["lives"] for p in s["players"]] == [4, 4]
assert [p["health"] for p in s["players"]] == [127, 127]
m = ResearchMachine(raw)
for slot in [0, 1]:
    for mask, field, sign in [
        (128, "x", 1),
        (16, "depth", 1),
        (32, "depth", -1),
        (3, "elevation", 1),
    ]:
        m.restore(initial)
        pair = [0, 0]
        pair[slot] = mask
        m.step(pair, 12)
        before = bytes(m.snapshot())
        f = json.loads(m.observe(task))
        assert bytes(m.snapshot()) == before
        ps = f["state"]["players"]
        assert sign * (ps[slot][field] - s["players"][slot][field]) > 0, (
            slot,
            mask,
            ps,
        )
        assert ps[1 - slot][field] == s["players"][1 - slot][field]
        assert all(len(o) == 256 and all(-1 <= v <= 1 for v in o) for o in f["observations"])
# Phase must advance across complete frames; the cartridge NMI flag at $f1
# resets each frame and cannot drive press/release timing.
m.restore(initial)
phases = []
pulses = []
for _ in range(256):
    f = json.loads(m.observe(task))
    phases.append(f["state"]["phase"])
    pulses.append((f["observations"][0][3], f["observations"][0][7]))
    m.step([0, 0], 1)
assert all((b - a) % 256 == 1 for a, b in zip(phases, phases[1:]))
assert len(set(phases)) == 256
assert {p[0] for p in pulses} == {0, 1} and {p[1] for p in pulses} == {0, 1}
m.restore(initial)
m.step([0, 0], 10)
m.step([8, 0], 4)
assert not json.loads(m.observe(task))["gameplay"]
f = json.loads(m.observe(task))["state"]["players"]
m.step([128, 3], 60)
assert json.loads(m.observe(task))["state"]["players"] == f
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
    "Double Dragon II: Normal co-op reset, independent movement and A+B jump, pause, read-only bounded fields and action rejection passed"
)
