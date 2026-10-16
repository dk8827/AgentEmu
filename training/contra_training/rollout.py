"""Uninterrupted evaluation of two controllers using the deployed Rust inference path."""

import hashlib
import json
from pathlib import Path
from ._native import Actor, Contra


def load_actor(directory):
    directory = Path(directory)
    return Actor(
        (directory / "manifest.json").read_text(), (directory / "weights.safetensors").read_bytes()
    )


def run_episode(core, controllers, seed, *, capture=False):
    """No restore, lookahead, extra lives, or teacher intervention in this rollout.

    A controller maps its current 256 structured features to a controller byte.
    Both decisions use the same pre-step frame. Like the player application,
    policies are inactive during the game's own exit animation.
    """
    step = json.loads(core.reset(seed))
    trace = bytearray()
    observations, actions = [], []
    reward = 0.0
    boss_frame = None
    while True:
        active = step["info"]["state"]["gameplay"]
        masks = [
            int(fn(o)) if active else 0
            for fn, o in zip(controllers, step["observations"], strict=True)
        ]
        if capture and active:
            observations.extend(step["observations"])
            actions.extend(masks)
        trace.extend(masks)
        step = json.loads(core.step(masks))
        info = step["info"]
        reward += sum(step["rewards"]) / 2
        if info["state"]["boss_defeated"] and boss_frame is None:
            boss_frame = info["frames"]
        if step["terminated"] or step["truncated"]:
            break
    result = dict(
        seed=seed,
        frames=info["frames"],
        success=info["success"],
        level=info["state"]["level"],
        boss_defeated_frame=boss_frame,
        truncated=step["truncated"],
        reward=reward,
        progress=info["max_progress"],
        deaths=info["deaths"],
        survivors=[p["active"] for p in info["state"]["players"]],
        input_sha256=hashlib.sha256(trace).hexdigest(),
        final_state_sha256=hashlib.sha256(bytes(core.snapshot())).hexdigest(),
    )
    assert result["success"] == (result["level"] == 1)
    return result, observations, actions, bytes(trace)


def evaluate_pair(rom, model, seeds, horizon, second_model=None):
    actors = [load_actor(model), load_actor(second_model or model)]
    core = Contra(Path(rom).read_bytes(), horizon)
    rows = []
    for seed in seeds:
        result, _, _, _ = run_episode(core, [a.buttons for a in actors], seed)
        rows.append(result)
        print(json.dumps(result), flush=True)
    return dict(
        protocol="contra-jungle-clear-v2",
        mode="two-neural-players",
        success_criterion="normal-play transition to CURRENT_LEVEL=1 (Base 1)",
        horizon=horizon,
        episodes=rows,
        clears=sum(r["success"] for r in rows),
    )
