"""Compare identical single-frame work through JSON, typed, and batched APIs.
Supply a local pinned Contra ROM. No ROM bytes or machine states are published.
"""

import argparse
import json
import statistics
import time
from pathlib import Path
from contra_training._native import Contra, EpisodeBatch
from contra_training.typed import unpack

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("rom", type=Path)
p.add_argument("--frames", type=int, default=180)
p.add_argument("--repeats", type=int, default=5)
a = p.parse_args()
if not 1 <= a.frames <= 180 or a.repeats < 1:
    p.error("frames must be 1..180; repeats must be positive")
rom = a.rom.read_bytes()
core = Contra(rom, a.frames)
batch = EpisodeBatch(rom, "contra-jungle", 4, a.frames)
results = {}
for kind in ("json", "typed-with-info", "typed-lazy-info", "batch-4-lazy-info"):
    times = []
    for repeat in range(a.repeats):
        if kind.startswith("batch"):
            batch.reset([repeat] * 4)
        else:
            core.reset(repeat)
        t = time.perf_counter()
        for frame in range(a.frames):
            masks = [130 | int(frame % 20 == 0), 2]
            if kind == "json":
                json.loads(core.step(masks))
            elif kind == "typed-with-info":
                unpack(core.step_data(masks))
            elif kind == "typed-lazy-info":
                core.step_data(masks).observations
            else:
                for s in batch.step([masks] * 4):
                    s.observations
        times.append(
            (time.perf_counter() - t) / (a.frames * (4 if kind.startswith("batch") else 1))
        )
    results[kind] = {
        "microseconds_per_environment_frame": statistics.median(times) * 1e6,
        "environment_frames_per_second": 1 / statistics.median(times),
    }
print(json.dumps({"frames": a.frames, "repeats": a.repeats, "results": results}, indent=2))
