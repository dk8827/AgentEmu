"""Evaluation and replay helpers; no training-framework dependency."""

import hashlib
import json
import numpy as np
from .env import buttons
from .tasks import teacher
from .contracts import CONTRACTS, RESETS
from .checkpoints import write_json


def rollout(
    core,
    task,
    warmup,
    actor=None,
    capture=False,
    controllers=None,
    teacher_profile="standard",
):
    step = json.loads(core.reset(warmup))
    xs = []
    ys = []
    inputs = []
    total = 0.0
    while True:
        obs = step["observations"]
        masks = []
        for slot, o in enumerate(obs):
            label = (
                teacher(task, o, teacher_profile)
                if capture or (actor is None and controllers is None)
                else None
            )
            masks.append(
                (
                    controllers[slot](o)
                    if controllers is not None
                    else actor.buttons(o)
                    if actor
                    else buttons(label)
                )
                if o[1] > 0.5
                else 0
            )
            if capture and o[1] > 0.5:
                xs.append(o)
                ys.append(label)
        inputs.append(masks)
        step = json.loads(core.step(masks))
        total += step["reward"]
        if step["terminated"] or step["truncated"]:
            break
    info = step["info"]
    state = info["state"]
    row = dict(
        warmup=warmup,
        profile="pair",
        slot=0,
        frames=info["frames"],
        success=info["success"],
        progress=info["max_progress"],
        deaths=info["deaths"],
        survivors=[p["active"] for p in state["players"]],
        reward=total,
        failure=(
            None
            if info["success"]
            else (
                "time-limit"
                if step["truncated"]
                else (
                    "match-lost"
                    if state.get("match_finished")
                    else (
                        "lives-exhausted"
                        if not any(p["active"] for p in state["players"])
                        else "unsupported-task"
                    )
                )
            )
        ),
        input_sha256=hashlib.sha256(bytes(v for pair in inputs for v in pair)).hexdigest(),
        final_state_sha256=hashlib.sha256(bytes(core.snapshot())).hexdigest(),
    )
    return row, inputs, xs, ys


def write_replay(path, task, row, inputs, entry_hash=None):
    data = dict(
        version=1,
        game=task,
        rom_sha256=CONTRACTS[task][2],
        reset=RESETS[task],
        warmup=row["warmup"],
        inputs=inputs,
        result=row,
    )
    if entry_hash:
        data["entry_state_sha256"] = entry_hash
    write_json(path, data, compact=True)


def summary(rows):
    return dict(
        episodes=len(rows),
        clears=sum(r["success"] for r in rows),
        both_survive=sum(r["success"] and all(r["survivors"]) for r in rows),
        zero_deaths=sum(r["success"] and not any(r["deaths"]) for r in rows),
        mean_deaths=float(np.mean([sum(r["deaths"]) for r in rows])),
        mean_progress=float(np.mean([r["progress"] for r in rows])),
    )
