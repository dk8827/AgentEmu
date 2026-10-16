"""Train/evaluate expansion agents; prepare Base 1 by an ordinary Jungle clear."""

import argparse, hashlib, json, time
from pathlib import Path
import numpy as np
import torch
from safetensors.torch import load_file
from ._native import Contra, ResearchMachine, TaskEpisode
from .env import buttons
from .tasks import teacher
from .model import ActorNet, export
from .contracts import CONTRACTS
from .rollout import load_actor
from .improve import fit
from .conditioning import feature_scale, runtime_actor, direction_class_weights
from .task_rollout import rollout, summary, write_replay


def prepare(rom, model, output):
    out = Path(output)
    out.mkdir(parents=True, exist_ok=False)
    raw = Path(rom).read_bytes()
    core = Contra(raw, 9000)
    actor = load_actor(model)
    step = json.loads(core.reset(0))
    trace = []
    while True:
        masks = [
            actor.buttons(o) if step["info"]["state"]["gameplay"] else 0
            for o in step["observations"]
        ]
        trace.append(masks)
        step = json.loads(core.step(masks))
        if step["terminated"] or step["truncated"]:
            break
    if not step["info"]["success"]:
        raise RuntimeError("Jungle entry policy failed; no Base 1 entry generated")
    m = ResearchMachine(raw)
    m.restore(bytes(core.snapshot()))
    for _ in range(1200):
        f = json.loads(m.observe("contra-base1"))["state"]
        if (
            f["gameplay"]
            and all(p["active"] and not p["dead"] and p["y"] == 168 for p in f["players"])
            and any(e["kind"] == 0x14 and e["hp"] == 8 for e in f["entities"])
        ):
            break
        m.step([0, 0], 1)
        trace.append([0, 0])
    else:
        raise RuntimeError("Base 1 did not become ready")
    snapshot = bytes(m.snapshot())
    (out / "entry.state").write_bytes(snapshot)
    (out / "provenance.json").write_text(
        json.dumps(
            dict(
                version=1,
                reset="contra-two-player-v1",
                rom_sha256=CONTRACTS["contra-base1"][2],
                model=json.loads((Path(model) / "manifest.json").read_text()),
                frames=len(trace),
                input_sha256=hashlib.sha256(bytes(v for pair in trace for v in pair)).hexdigest(),
                state_sha256=hashlib.sha256(snapshot).hexdigest(),
            ),
            indent=2,
        )
        + "\n"
    )
    (out / "inputs.json").write_text(json.dumps(trace, separators=(",", ":")) + "\n")
    print("Prepared Base 1 entry through a normal Jungle clear:", out, flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("command", choices=["prepare-base", "train", "evaluate"])
    p.add_argument(
        "--task",
        choices=[
            "contra-base1",
            "snowbros-floor1",
            "bubblebobble-round1",
            "iceclimber-mountain1",
            "rivercity-moose",
            "tmnt2-scene1",
            "doubledragon2-mission1",
            "worldcup-firstmatch",
            "chipndale-zone0",
            "jackal-stage1",
            "lifeforce-stage1",
        ],
    )
    p.add_argument("--rom", required=True)
    p.add_argument("--output", required=True)
    p.add_argument("--model")
    p.add_argument("--resume", help="continue a validated task bundle with a fresh optimizer")
    p.add_argument("--condition-observations", action="store_true")
    p.add_argument("--balance-directions", action="store_true")
    p.add_argument("--entry")
    p.add_argument("--rounds", type=int, default=6)
    p.add_argument("--epochs", type=int, default=40)
    p.add_argument("--seed", type=int, default=83)
    p.add_argument(
        "--teacher-profile",
        choices=("standard", "punch-edges", "walk-and-punch", "lateral-jumps"),
        default="standard",
        help="Optional game-specific teaching controller; standard remains the comparison baseline",
    )
    p.add_argument("--horizon", type=int, default=12000)
    p.add_argument("--warmups", default="97,113,137,163,191,223")
    a = p.parse_args()
    if (
        a.teacher_profile != "standard"
        and {
            "punch-edges": "rivercity-moose",
            "walk-and-punch": "rivercity-moose",
            "lateral-jumps": "doubledragon2-mission1",
        }.get(a.teacher_profile)
        != a.task
    ):
        p.error("teacher profile is not supported for this task")
    try:
        warmups = [int(value) for value in a.warmups.split(",")]
        if not warmups or any(not 0 <= value <= 240 for value in warmups):
            raise ValueError()
    except ValueError:
        p.error("--warmups must contain comma-separated integers in 0..240")
    if a.command == "prepare-base":
        if not a.model:
            p.error("prepare-base needs --model")
        prepare(a.rom, a.model, a.output)
        return
    if not a.task:
        p.error("--task is required")
    if a.task == "contra-base1" and not a.entry:
        p.error("Base 1 needs --entry from prepare-base")
    if a.command == "train" and a.task == "iceclimber-mountain1":
        p.error("This task uses demonstration training; use trajectory_train")
    core = TaskEpisode(
        Path(a.rom).read_bytes(),
        a.task,
        a.horizon,
        Path(a.entry).read_bytes() if a.entry else None,
    )
    entry_hash = hashlib.sha256(Path(a.entry).read_bytes()).hexdigest() if a.entry else None
    out = Path(a.output)
    out.mkdir(parents=True, exist_ok=False)
    (out / "configuration.json").write_text(json.dumps(vars(a), indent=2) + "\n")
    if a.entry:
        (out / "entry-hash.json").write_text(
            json.dumps(dict(sha256=hashlib.sha256(Path(a.entry).read_bytes()).hexdigest())) + "\n"
        )
    if a.command == "evaluate":
        if not a.model:
            p.error("evaluate needs --model")
        actor = load_actor(a.model)
        rows = []
        if (
            json.loads((Path(a.model) / "manifest.json").read_text())["observation"]
            != CONTRACTS[a.task][1]
        ):
            raise ValueError("model/task mismatch")
        for warmup in warmups:
            row, inputs, _, _ = rollout(core, a.task, warmup, actor)
            rows.append(row)
            (out / f"inputs-{warmup}.json").write_text(json.dumps(inputs, separators=(",", ":")))
            write_replay(out / f"replay-{warmup}.json", a.task, row, inputs, entry_hash)
            print(json.dumps(row), flush=True)
        report = dict(
            version=1,
            task=a.task,
            suite=a.task + "-timing-v1",
            split="test",
            model=json.loads((Path(a.model) / "manifest.json").read_text()),
            summary=summary(rows),
            episodes=rows,
        )
        (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        return
    torch.set_num_threads(1)
    torch.manual_seed(a.seed)
    np.random.seed(a.seed)
    torch.use_deterministic_algorithms(True)
    net = ActorNet()
    scale = feature_scale(a.task) if a.condition_observations else np.ones(256, np.float32)
    resume_hash = None
    if a.resume:
        load_actor(a.resume)
        manifest = json.loads((Path(a.resume) / "manifest.json").read_text())
        if (
            manifest["architecture"],
            manifest["observation"],
            manifest["rom_sha256"],
        ) != CONTRACTS[a.task]:
            p.error("resume bundle belongs to a different task")
        net.load_state_dict(load_file(Path(a.resume) / "weights.safetensors"))
        with torch.no_grad():
            net.layers[0].weight.div_(torch.as_tensor(scale))
        resume_hash = manifest["weights_sha256"]
    opt = torch.optim.Adam(net.parameters(), lr=0.0005)
    xs = []
    ys = []
    teachers = []
    history = []
    best = None
    bestkey = (-1, -1, -1, -1e9)
    started = time.perf_counter()
    for warmup in [0, 8, 24, 48, 72, 96]:
        row, inputs, x, y = rollout(
            core, a.task, warmup, capture=True, teacher_profile=a.teacher_profile
        )
        xs.extend(x)
        ys.extend(y)
        teachers.append(row)
    (out / "teacher.json").write_text(json.dumps(teachers, indent=2) + "\n")
    xs = np.asarray(xs, np.float32)
    ys = np.asarray(ys, np.int64)
    for rnd in range(a.rounds):
        if rnd:
            actor = load_actor(out / f"round-{rnd - 1:02d}")
            newx = []
            newy = []
            for warmup in [(rnd * 13 + i * 17) % 97 for i in range(4)]:
                _, _, x, y = rollout(
                    core,
                    a.task,
                    warmup,
                    actor,
                    capture=True,
                    teacher_profile=a.teacher_profile,
                )
                newx.extend(x)
                newy.extend(y)
            xs = np.concatenate((xs, np.asarray(newx, np.float32)))
            ys = np.concatenate((ys, np.asarray(newy, np.int64)))
        metrics = fit(
            net,
            xs * scale,
            ys,
            a.epochs,
            opt,
            direction_class_weights(ys) if a.balance_directions else None,
        )
        directory = out / f"round-{rnd:02d}"
        export(
            runtime_actor(net, scale),
            directory,
            dict(
                name=a.task + " research candidate",
                algorithm="observation-only teacher + DAgger",
                teacher_profile=a.teacher_profile,
                resume_weights_sha256=resume_hash,
                learning_feature_scale=scale.tolist(),
                task=a.task,
                round=rnd,
                seed=a.seed,
                examples=len(xs),
                epochs=a.epochs,
                fit=metrics,
            ),
            xs[:: max(1, len(xs) // 256)],
            task=a.task,
        )
        actor = load_actor(directory)
        rows = []
        for warmup in [0, 16, 40, 80]:
            row, inputs, _, _ = rollout(core, a.task, warmup, actor)
            rows.append(row)
            (directory / f"inputs-{warmup}.json").write_text(
                json.dumps(inputs, separators=(",", ":"))
            )
            write_replay(directory / f"replay-{warmup}.json", a.task, row, inputs, entry_hash)
        metrics_summary = summary(rows)
        key = (
            metrics_summary["clears"],
            metrics_summary["both_survive"],
            metrics_summary["mean_progress"],
            -metrics_summary["mean_deaths"],
        )
        if key > bestkey:
            best = directory
            bestkey = key
        report = dict(
            version=1,
            task=a.task,
            suite=a.task + "-timing-v1",
            split="selection",
            model=json.loads((directory / "manifest.json").read_text()),
            round=rnd,
            summary=metrics_summary,
            fit=metrics,
            episodes=rows,
            elapsed_seconds=time.perf_counter() - started,
        )
        history.append(report)
        (directory / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        (out / "history.json").write_text(json.dumps(history, indent=2) + "\n")
        np.savez_compressed(out / "dataset.npz", observations=xs, actions=ys)
        (out / "result.json").write_text(
            json.dumps(
                dict(
                    best=str(best),
                    summary=history[int(best.name.split("-")[-1])]["summary"],
                ),
                indent=2,
            )
            + "\n"
        )
        print(
            json.dumps(dict(round=rnd, summary=metrics_summary, best=str(best))),
            flush=True,
        )
        if metrics_summary["clears"] == 4 and metrics_summary["both_survive"] == 4:
            break


if __name__ == "__main__":
    main()
