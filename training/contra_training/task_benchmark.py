"""Reproducible expansion-task baselines and mixed-controller evaluations."""

import argparse
import hashlib
import json
from pathlib import Path
import numpy as np
from ._native import TaskEpisode
from .contracts import CONTRACTS
from .env import buttons
from .rollout import load_actor
from .tasks import teacher
from .task_rollout import rollout, summary, write_replay

PROFILES = ("pair", "teacher", "random", "idle", "agent-p1", "agent-p2")


def scripted_partner(suite, task):
    profile = suite.get("scripted_partners", {}).get(task, "standard")
    if task == "contra-jungle":
        if profile != "standard":
            raise ValueError("Contra uses its versioned route partner")
    else:
        teacher(task, np.zeros(256), profile)
    return profile


def controllers(task, profile, actor, seed, *, teacher_profile="standard"):
    if profile not in PROFILES:
        raise ValueError("unknown controller profile")
    rngs = [np.random.default_rng(np.random.SeedSequence([seed, p])) for p in range(2)]

    def teach(o):
        return buttons(teacher(task, o, teacher_profile))

    random = [lambda o, rng=rng: buttons(rng.integers([0, 0, 0, 0], [3, 3, 2, 2])) for rng in rngs]
    return {
        "pair": [actor.buttons, actor.buttons],
        "teacher": [teach, teach],
        "random": random,
        "idle": [lambda o: 0, lambda o: 0],
        "agent-p1": [actor.buttons, teach],
        "agent-p2": [teach, actor.buttons],
    }[profile]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--task", choices=CONTRACTS, required=True)
    p.add_argument("--rom", required=True)
    p.add_argument("--model", required=True)
    p.add_argument("--entry")
    p.add_argument("--suite", required=True)
    p.add_argument("--output", required=True)
    a = p.parse_args()
    suite = json.loads(Path(a.suite).read_text())
    if suite.get("version") != 1 or suite.get("task") != a.task:
        p.error("suite/task mismatch")
    cases = suite.get("cases", [])
    if not cases or len(cases) > 256 or not 1 <= suite.get("horizon", 0) <= 108000:
        p.error("invalid suite bounds")
    for c in cases:
        if (
            c.get("profile") not in PROFILES
            or not isinstance(c.get("warmup"), int)
            or not 0 <= c["warmup"] <= 240
            or not isinstance(c.get("seed"), int)
            or c["seed"] < 0
        ):
            p.error("invalid scenario")
    model = Path(a.model)
    manifest = json.loads((model / "manifest.json").read_text())
    if manifest["observation"] != CONTRACTS[a.task][1]:
        p.error("model/task mismatch")
    actor = load_actor(model)
    entry = Path(a.entry).read_bytes() if a.entry else None
    core = TaskEpisode(Path(a.rom).read_bytes(), a.task, suite["horizon"], entry)
    out = Path(a.output)
    out.mkdir(parents=True, exist_ok=False)
    (out / "configuration.json").write_text(json.dumps(vars(a), indent=2) + "\n")
    (out / "suite.json").write_text(json.dumps(suite, indent=2) + "\n")
    rows = []
    for i, c in enumerate(cases):
        row, trace, _, _ = rollout(
            core,
            a.task,
            c["warmup"],
            controllers=controllers(a.task, c["profile"], actor, c["seed"]),
        )
        row.update(
            profile=c["profile"],
            slot=1 if c["profile"] == "agent-p2" else 0,
            seed=c["seed"],
        )
        rows.append(row)
        write_replay(
            out / f"replay-{i:03d}.json",
            a.task,
            row,
            trace,
            hashlib.sha256(entry).hexdigest() if entry else None,
        )
        report = dict(
            version=1,
            task=a.task,
            suite=suite["name"],
            split=suite["split"],
            model=manifest,
            suite_sha256=hashlib.sha256(Path(a.suite).read_bytes()).hexdigest(),
            summary=summary(rows),
            by_profile={
                profile: summary([r for r in rows if r["profile"] == profile])
                for profile in PROFILES
                if any(r["profile"] == profile for r in rows)
            },
            episodes=rows,
        )
        (out / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(
            json.dumps(
                dict(
                    case=i,
                    profile=c["profile"],
                    warmup=c["warmup"],
                    clear=row["success"],
                    frames=row["frames"],
                )
            ),
            flush=True,
        )
    print(json.dumps(report["by_profile"]), flush=True)


if __name__ == "__main__":
    main()
