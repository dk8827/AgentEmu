"""Versioned Jungle teamwork scenarios and ROM-free controller replays."""

import argparse, hashlib, json
from pathlib import Path
import numpy as np
from ._native import Contra
from .contracts import ROM
from .rollout import load_actor
from .teacher import action as route_action
from .env import buttons

PROFILES = ("pair", "steady", "pause", "retreat", "jumper")


def cooperative_action_v1(o):
    a = route_action(o)
    # Wait for a live lagging teammate; don't drag a respawning partner off screen.
    if o[24] > 0.5 and o[30] < 0.5 and o[9] - o[25] > 64 / 255 and o[4] * 4096 < 2970:
        a[0] = 0
    return a


def cooperative_action(o):
    a = route_action(o)
    # Only wait on safe footing. Never interrupt a jump or wait in an approaching threat.
    threats = [o[40 + i * 7 : 47 + i * 7] for i in range(8)]
    safe = not any(e[0] > 0.5 and abs(e[1] * 256) < 64 and abs(e[2] * 240) < 32 for e in threats)
    if (
        o[24] > 0.5
        and o[30] < 0.5
        and o[13] > 0.5
        and o[29] > 0.5
        and safe
        and o[9] - o[25] > 64 / 255
        and o[4] * 4096 < 2970
    ):
        a[0] = 0
    return a


def teammate(o, frame, profile):
    a = route_action(o)
    # Delays are bounded and declared, not a claim to represent all human play.
    if profile == "pause" and 120 <= frame % 480 < 180:
        a[0] = 0
        a[2] = 0
    elif profile == "retreat" and 160 <= frame % 600 < 180:
        a[0] = 1
    elif profile == "jumper":
        a[2] = int(o[13] > 0.5 and o[4] * 4096 < 2970)
    return buttons(a)


def scenario(
    core,
    actor,
    case,
    horizon,
    *,
    teacher=False,
    capture=False,
    labeler=cooperative_action,
):
    warmup, profile, slot = case["warmup"], case["profile"], case["slot"]
    step = json.loads(core.reset_timing(warmup))
    masks_trace, telemetry, xs, ys = [], [], [], []
    total = 0.0
    last_progress = 0
    last_advance = 0
    boss_frame = None
    while True:
        info = step["info"]
        state = info["state"]
        frame = info["frames"]
        masks = [0, 0]
        if state["gameplay"]:
            for port, o in enumerate(step["observations"]):
                controlled = profile == "pair" or port == slot
                if controlled:
                    masks[port] = buttons(labeler(o)) if teacher else actor.buttons(o)
                    if capture:
                        xs.append(o)
                        ys.append(labeler(o))
                else:
                    masks[port] = teammate(o, frame, profile)
        masks_trace.append(masks)
        step = json.loads(core.step(masks))
        info = step["info"]
        state = info["state"]
        total += sum(step["rewards"]) / 2
        if max(info["max_progress"]) > last_progress:
            last_progress = max(info["max_progress"])
            last_advance = info["frames"]
        if state["boss_defeated"] and boss_frame is None:
            boss_frame = info["frames"]
        if info["frames"] % 60 == 0 or step["terminated"] or step["truncated"]:
            telemetry.append(
                dict(
                    frame=info["frames"],
                    camera=state["camera_x"],
                    players=[
                        dict(
                            x=p["x"],
                            y=p["y"],
                            active=p["active"],
                            dead=p["dead"],
                            lives=p["reserve_lives"],
                        )
                        for p in state["players"]
                    ],
                )
            )
        if step["terminated"] or step["truncated"]:
            break
    row = dict(
        **case,
        frames=info["frames"],
        success=info["success"],
        level=state["level"],
        boss_defeated_frame=boss_frame,
        deaths=info["deaths"],
        progress=info["max_progress"],
        survivors=[p["active"] for p in state["players"]],
        reward=total,
        failure=None
        if info["success"]
        else "lives-exhausted"
        if step["terminated"]
        else "stalled"
        if info["frames"] - last_advance > 600
        else "time-limit",
        input_sha256=hashlib.sha256(bytes(v for pair in masks_trace for v in pair)).hexdigest(),
        final_state_sha256=hashlib.sha256(bytes(core.snapshot())).hexdigest(),
    )
    replay = dict(
        version=1,
        game="contra-jungle",
        rom_sha256=ROM,
        reset="contra-two-player-v1",
        warmup=warmup,
        inputs=masks_trace,
        telemetry=telemetry,
        result=row,
    )
    return row, replay, xs, ys


def summarize(rows):
    return dict(
        episodes=len(rows),
        clears=sum(r["success"] for r in rows),
        both_survive=sum(r["success"] and all(r["survivors"]) for r in rows),
        zero_deaths=sum(r["success"] and not any(r["deaths"]) for r in rows),
        mean_deaths=float(np.mean([sum(r["deaths"]) for r in rows])) if rows else 0.0,
    )


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--rom", required=True)
    p.add_argument("--model", required=True)
    p.add_argument("--suite", required=True)
    p.add_argument("--split", choices=["selection", "test", "legacy"], default="test")
    p.add_argument("--output", required=True)
    p.add_argument("--horizon", type=int, default=9000)
    args = p.parse_args()
    out = Path(args.output)
    out.mkdir(parents=True, exist_ok=False)
    suite = json.loads(Path(args.suite).read_text())
    core = Contra(Path(args.rom).read_bytes(), args.horizon)
    actor = load_actor(args.model)
    rows = []
    for index, case in enumerate(suite[args.split]):
        row, replay, _, _ = scenario(core, actor, case, args.horizon)
        rows.append(row)
        path = f"replay-{index:03d}.json"
        (out / path).write_text(json.dumps(replay, separators=(",", ":")) + "\n")
        row["replay"] = path
        report = dict(
            version=1,
            model=json.loads((Path(args.model) / "manifest.json").read_text()),
            suite=suite["id"],
            split=args.split,
            summary=summarize(rows),
            episodes=rows,
        )
        (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(row), flush=True)
    print(json.dumps(summarize(rows)), flush=True)


if __name__ == "__main__":
    main()
