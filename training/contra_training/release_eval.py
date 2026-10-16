"""Frozen release evaluation for the six bundled games; no training dependencies."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

import numpy as np

from ._native import Contra, TaskEpisode
from .benchmark import scenario, summarize, cooperative_action
from .env import buttons
from .release_contract import check_death_metric
from .contracts import CONTRACTS
from .checkpoints import write_json
from .rollout import load_actor
from .task_benchmark import controllers, scripted_partner
from .task_rollout import rollout, write_replay


def acceptance(episodes, baseline_rows, minimum, expected_cases=None):
    by_profile = {
        p: summarize([r for r in episodes if r["profile"] == p])
        for p in ("pair", "agent-p1", "agent-p2")
    }
    reliable = all(
        v["episodes"] and v["clears"] / v["episodes"] >= minimum for v in by_profile.values()
    )
    pair = by_profile["pair"]
    expected_cases = expected_cases or pair["episodes"]
    complete = expected_cases > 0 and all(
        sum(r["profile"] == profile for r in episodes + baseline_rows) == expected_cases
        for profile in ("pair", "agent-p1", "agent-p2", "teacher", "random", "idle")
    )

    def score(rows):
        s = summarize(rows)
        n = max(1, s["episodes"])
        return (
            s["clears"] / n,
            s["both_survive"] / n,
            -s["mean_deaths"],
            -sum(r["frames"] for r in rows) / n,
        )

    # A tied clear rate can still improve survival, deaths or completion time.
    comparisons = {
        p: score([r for r in episodes if r["profile"] == "pair"]) > score(rows)
        for p in ("random", "idle")
        if (rows := [r for r in baseline_rows if r["profile"] == p])
    }
    return {
        "by_profile": by_profile,
        "reliable": reliable,
        "better_than_baselines": comparisons,
        "complete": complete,
        "performance_pass": complete
        and reliable
        and len(comparisons) == 2
        and all(comparisons.values()),
        "deployment_parity": "pending",
        "human_shared_play": "not-assessed",
    }


class JungleBaseline:
    def __init__(self, profile, seed):
        self.profile = profile
        self.rng = [np.random.default_rng(np.random.SeedSequence([seed, p])) for p in range(2)]

    def buttons(self, observation):
        if self.profile == "idle":
            return 0
        if self.profile == "teacher":
            return buttons(cooperative_action(observation))
        return buttons(self.rng[int(observation[0] > 0.5)].integers([0, 0, 0, 0], [3, 3, 2, 2]))


def read_resume(path, task, split, manifest, suite, suite_digest, partner):
    report = json.loads(path.read_bytes())
    for key, value in dict(
        task=task, split=split, model=manifest, suite_sha256=suite_digest, scripted_partner=partner
    ).items():
        if report.get(key) != value:
            raise ValueError("resume protocol mismatch: " + key)
    rows = report["episodes"] + report["baselines"]
    expected = [
        (p, w, suite["seed"] + i)
        for p in suite["profiles"] + suite["baselines"]
        for i, w in enumerate(suite[split])
    ]
    if [(r["profile"], r["warmup"], r["seed"]) for r in rows] != expected[: len(rows)] or len(
        rows
    ) > len(expected):
        raise ValueError("resume cases are not a prefix of the frozen suite")
    if any(r["profile"] not in suite["profiles"] for r in report["episodes"]) or any(
        r["profile"] not in suite["baselines"] for r in report["baselines"]
    ):
        raise ValueError("resume neural and baseline cases are mixed")
    for row in rows:
        if Path(row["replay"]).name != row["replay"]:
            raise ValueError("resume replay must be a sibling file")
        replay = json.loads((path.parent / row["replay"]).read_bytes())
        if (
            replay["result"] != row
            or replay["rom_sha256"] != manifest["rom_sha256"]
            or len(replay["inputs"]) != row["frames"]
            or hashlib.sha256(bytes(v for pair in replay["inputs"] for v in pair)).hexdigest()
            != row["input_sha256"]
        ):
            raise ValueError("resume replay is incomplete or corrupt")
    return report


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--rom", required=True)
    p.add_argument("--model", required=True)
    p.add_argument("--task", required=True)
    p.add_argument("--suite", default="fixtures/benchmarks/agentemu-release-v1.json")
    p.add_argument("--split", choices=["selection", "test"], required=True)
    p.add_argument("--output", required=True)
    p.add_argument("--resume", action="store_true", help="verify and continue a partial evaluation")
    a = p.parse_args()
    suite_bytes = Path(a.suite).read_bytes()
    suite = json.loads(suite_bytes)
    check_death_metric(suite, a.task)
    partner_profile = scripted_partner(suite, a.task)
    if a.task not in suite["horizons"] or set(suite["selection"]) & set(suite["test"]):
        p.error("Invalid task or overlapping selection/test starts")
    raw = Path(a.rom).read_bytes()
    manifest = json.loads((Path(a.model) / "manifest.json").read_text())
    if manifest["observation"] != CONTRACTS[a.task][1]:
        p.error("Model does not match the task observation contract")
    if hashlib.sha256(raw).hexdigest() != manifest["rom_sha256"]:
        p.error("Cartridge does not match the model")
    out = Path(a.output)
    out.mkdir(parents=True, exist_ok=a.resume)
    actor = load_actor(a.model)
    horizon = suite["horizons"][a.task]
    core = Contra(raw, horizon) if a.task == "contra-jungle" else TaskEpisode(raw, a.task, horizon)
    root = Path(__file__).resolve().parents[2]
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    dirty = bool(
        subprocess.check_output(
            ["git", "status", "--porcelain", "--untracked-files=no"], cwd=root, text=True
        ).strip()
    )
    rows, baselines = [], []
    resume_history = []
    report = None
    if a.resume:
        report = read_resume(
            out / "report.json",
            a.task,
            a.split,
            manifest,
            suite,
            hashlib.sha256(suite_bytes).hexdigest(),
            partner_profile,
        )
        rows, baselines = report["episodes"], report["baselines"]
        resume_history = report.get("resume_history", []) + [
            dict(source_commit=commit, source_dirty=dirty)
        ]
        # A run spanning source revisions is useful research evidence, but is not
        # a clean frozen-revision release run. Preserve that distinction.
        dirty = dirty or report["source_dirty"] or commit != report["source_commit"]
        commit = report["source_commit"]
    completed = {(r["profile"], r["warmup"]) for r in rows + baselines}
    for profile in suite["profiles"] + suite["baselines"]:
        for index, warmup in enumerate(suite[a.split]):
            if (profile, warmup) in completed:
                continue
            seed = suite["seed"] + index
            if a.task == "contra-jungle":
                baseline = profile in suite["baselines"]
                chosen = JungleBaseline(profile, seed) if baseline else actor
                case = {
                    "warmup": warmup,
                    "profile": "pair" if baseline or profile == "pair" else "steady",
                    "slot": int(profile == "agent-p2"),
                }
                row, replay, _, _ = scenario(core, chosen, case, horizon)
                row["profile"] = profile
                replay["result"] = row
            else:
                row, trace, _, _ = rollout(
                    core,
                    a.task,
                    warmup,
                    controllers=controllers(
                        a.task, profile, actor, seed, teacher_profile=partner_profile
                    ),
                )
                row.update(profile=profile, slot=int(profile == "agent-p2"))
                replay = None
            row["seed"] = seed
            row["replay"] = f"{profile}-{warmup}.json"
            if replay is not None:
                write_json(out / row["replay"], replay, compact=True)
            else:
                write_replay(out / row["replay"], a.task, row, trace, None)
            (baselines if profile in suite["baselines"] else rows).append(row)
            report = {
                "version": 1,
                "task": a.task,
                "split": a.split,
                "scripted_partner": partner_profile,
                "source_commit": commit,
                "source_dirty": dirty,
                "resume_history": resume_history,
                "suite_sha256": hashlib.sha256(suite_bytes).hexdigest(),
                "model": manifest,
                "episodes": rows,
                "baselines": baselines,
                "acceptance": acceptance(
                    rows, baselines, suite["minimum_clear_rate"], len(suite[a.split])
                ),
            }
            write_json(out / "report.json", report)
            print(
                json.dumps(
                    {
                        "profile": profile,
                        "warmup": warmup,
                        "success": row["success"],
                        "deaths": row["deaths"],
                        "frames": row["frames"],
                    }
                ),
                flush=True,
            )
    print(json.dumps(report["acceptance"]), flush=True)


if __name__ == "__main__":
    main()
