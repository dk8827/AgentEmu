"""Local-ROM semantic checks. Reports facts only; no ROM or RAM is published."""

import argparse
import json
from pathlib import Path
import numpy as np
from contra_training._native import Contra
from contra_training.env import ContraEnv, buttons, scripted
from gymnasium.utils.env_checker import check_env

p = argparse.ArgumentParser()
p.add_argument("rom")
p.add_argument("--output", default=".cache/contra-validation.json")
a = p.parse_args()
env = ContraEnv(a.rom, horizon=300)
check_env(env, skip_render_check=True)
e = Contra(Path(a.rom).read_bytes(), 7200)


def reset(seed=0):
    return json.loads(e.reset(seed))


def step(m):
    return json.loads(e.step(m))


r = reset()
initial = r["info"]["state"]
for _ in range(20):
    r = step([128, 0])
after = r["info"]["state"]
assert after["players"][0]["x"] > initial["players"][0]["x"]
assert after["players"][1]["x"] == initial["players"][1]["x"]
right_delta = after["players"][0]["x"] - initial["players"][0]["x"]
r = reset()
for i in range(12):
    r = step([1 if i == 0 else 0, 0])
jump = r["info"]["state"]
assert jump["players"][0]["y"] < initial["players"][0]["y"] and not jump["players"][0]["grounded"]
assert jump["players"][1]["grounded"]
# Weapon fire must create player-bullet objects; B is distinct from A.
r = reset()
for i in range(12):
    r = step([2 if i % 2 == 0 else 0, 0])
ram = e.ram()
# Bullet sprite slots follow the two player sprites in CPU_SPRITE_BUFFER.
assert any(ram[0x302:0x30A])
# Current floor under both players matches the local collision grid (row 3, column 2).
for pidx in (0, 1):
    assert initial["terrain"][pidx][3 * 8 + 2] in (1, 3)


# Deterministic reset + exact same action sequence gives identical machine state.
def replay(seed):
    r = reset(seed)
    for _ in range(180):
        r = step([buttons(scripted(o)) for o in r["observations"]])
    return bytes(e.snapshot()), r


snap, r = replay(13)
snap2, r2 = replay(13)
assert snap == snap2 and r == r2
# Planning restores must include reward/death accounting, not only emulator RAM.
checkpoint = e.checkpoint()
branch = [step([128 | (i % 2) * 2, 128 | (i % 3 == 0)]) for i in range(90)]
branch_snapshot = bytes(e.snapshot())
e.restore(checkpoint)
assert branch == [step([128 | (i % 2) * 2, 128 | (i % 3 == 0)]) for i in range(90)]
assert branch_snapshot == bytes(e.snapshot())
r = reset()
camera_max = 0
deaths = 0
entities = set()
observations = []
for i in range(2400):
    observations.append(r["observations"])
    r = step([buttons(scripted(o)) for o in r["observations"]])
    state = r["info"]["state"]
    camera_max = max(camera_max, state["camera_x"])
    entities.update(x["kind"] for x in state["entities"])
    deaths = sum(r["info"]["deaths"])
    if r["terminated"] or r["truncated"]:
        break
assert camera_max > 256 and deaths > 0 and 1 in entities
assert np.isfinite(observations).all()
# A time limit is truncation, not an in-game terminal event.
t = Contra(Path(a.rom).read_bytes(), 1)
t.reset(0)
end = json.loads(t.step([0, 0]))
assert end["truncated"] and not end["terminated"]
try:
    t.step([0, 0])
    raise AssertionError("step after done accepted")
except ValueError:
    pass
finished = t.checkpoint()
t.reset(0)
restored = json.loads(t.restore(finished))
assert restored["truncated"] and not restored["terminated"]
try:
    t.step([0, 0])
    raise AssertionError("restored terminal checkpoint accepted step")
except ValueError:
    pass
result = dict(
    gymnasium_check="passed",
    independent_p1_motion_pixels=right_delta,
    jump_y=jump["players"][0]["y"],
    initial_y=initial["players"][0]["y"],
    fire_objects="passed",
    floor_alignment="passed",
    deterministic_reset_replay="passed",
    camera_max=camera_max,
    deaths=deaths,
    entity_types=sorted(entities),
    time_limit_semantics="passed",
    scope="US Jungle start and early gameplay only",
)
out = Path(a.output)
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))
