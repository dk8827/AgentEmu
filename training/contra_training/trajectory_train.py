"""Train fixed neural policies from locally verified controller demonstrations.

Demonstrations are replay JSON, never executable checkpoints or save-state patches.
A nearest-demonstration teacher labels rollout states during DAgger. It is used
only in training; exported play uses the ordinary Rust/WASM fixed neural actor.
"""

import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
import torch
from safetensors.torch import load_file

from ._native import TaskEpisode
from .contracts import CONTRACTS, RESETS
from .improve import fit
from .model import ActorNet, export
from .rollout import load_actor
from .task_rollout import rollout, summary, write_replay
from .tasks import teacher
from .conditioning import feature_scale, runtime_actor, direction_class_weights


def factors(mask):
    if (
        type(mask) is not int
        or not 0 <= mask <= 255
        or mask & 12
        or mask & 48 == 48
        or mask & 192 == 192
    ):
        raise ValueError("demonstration contains invalid training input")
    return [
        2 if mask & 128 else 1 if mask & 64 else 0,
        2 if mask & 32 else 1 if mask & 16 else 0,
        mask & 1,
        (mask >> 1) & 1,
    ]


def collect(core, task, replay):
    if (
        replay.get("version") != 1
        or replay.get("game") != task
        or replay.get("rom_sha256") != CONTRACTS[task][2]
    ):
        raise ValueError("demonstration contract mismatch")
    resets = RESETS
    if replay.get("reset") != resets[task]:
        raise ValueError("demonstration reset mismatch")
    warmup = replay.get("warmup")
    if type(warmup) is not int or not 0 <= warmup <= 240:
        raise ValueError("invalid demonstration warmup")
    training_start = replay.get("result", {}).get("training_start_frame", 0)
    inputs = replay.get("inputs")
    if not isinstance(inputs, list) or not 1 <= len(inputs) <= 108000:
        raise ValueError("demonstration must have 1..108000 frames")
    if type(training_start) is not int or not 0 <= training_start < len(inputs):
        raise ValueError("invalid demonstration training start")
    labels = []
    for pair in inputs:
        if not isinstance(pair, list) or len(pair) != 2:
            raise ValueError("demonstration needs two controller ports")
        labels.append([factors(mask) for mask in pair])
    digest = hashlib.sha256(bytes(v for pair in inputs for v in pair)).hexdigest()
    if digest != replay.get("result", {}).get("input_sha256"):
        raise ValueError("demonstration input digest mismatch")
    step = json.loads(core.reset(warmup))
    xs, ys = [], []
    profile = replay.get("result", {}).get("profile")
    slots = (0,) if profile == "agent-p1" else (1,) if profile == "agent-p2" else (0, 1)
    for index, (pair, label) in enumerate(zip(inputs, labels)):
        if not step["info"]["state"]["gameplay"] and pair != [0, 0]:
            raise ValueError("demonstration must use neutral input outside gameplay")
        for slot, o in enumerate(step["observations"]):
            if index >= training_start and slot in slots and o[1] > 0.5:
                xs.append(o)
                ys.append(label[slot])
        step = json.loads(core.step(pair))
        if (step["terminated"] or step["truncated"]) and index != len(inputs) - 1:
            raise ValueError("demonstration continues beyond episode end")
    expected = replay["result"]
    if not step["info"]["success"] or not expected.get("success"):
        raise ValueError("demonstration must finish the task")
    if hashlib.sha256(bytes(core.snapshot())).hexdigest() != expected.get("final_state_sha256"):
        raise ValueError("demonstration final emulator state mismatch")
    return np.asarray(xs, np.float32), np.asarray(ys, np.int64)


class FloorDemonstration:
    """Ice Climber supervision that never borrows a route from another floor.

    Used only during training. Grouping also avoids a full trajectory scan for
    every airborne frame; the cartridge's momentum determines that label.
    """

    def __init__(self, xs, ys):
        self.weights = np.ones(256, np.float32)
        self.weights[0] = 16
        self.weights[2:4] = 0.25
        self.weights[6] = 255
        self.weights[40:232] = 0.1
        for base in (8, 24):
            self.weights[base + 1 : base + 3] = 8
            self.weights[base + 6 : base + 10] = [255, 64, 128, 128]
        self.groups = {}
        for slot in (0, 1):
            for floor in np.unique(np.rint(xs[:, 14] * 255).astype(int)):
                ix = (xs[:, 0] == slot) & (np.rint(xs[:, 14] * 255) == floor) & (xs[:, 13] > 0.5)
                if ix.any():
                    self.groups[slot, floor] = (xs[ix] * self.weights, ys[ix])

    def label(self, observation):
        if observation[13] < 0.5:
            return [{1: 2, 2: 1}.get(round(observation[16] * 255), 0), 0, 0, 0]
        group = self.groups.get((int(observation[0] > 0.5), round(observation[14] * 255)))
        horizontal = 0
        if group is not None:
            points, labels = group
            diff = points - np.asarray(observation, np.float32) * self.weights
            horizontal = int(labels[np.einsum("ij,ij->i", diff, diff).argmin(), 0])
        return [horizontal, 0, int(observation[20] < 0.5), 0]


class NearestDemonstration:
    def __init__(self, xs, ys, task, grounded=False, route_controls=False, combat_controls=False):
        self.task = task
        self.combat_controls = combat_controls and task == "rivercity-moose"
        self.route_controls = route_controls and task == "rivercity-moose"
        self.grounded = grounded and task == "iceclimber-mountain1"
        self.weights = np.ones(256, np.float32)
        self.weights[0] = 16  # Keep the two roles distinct.
        self.weights[2:4] = 0.25
        if task == "lifeforce-stage1":
            self.weights[40:160] = 2
            self.weights[160:210] = 4
            for base in (8, 24):
                self.weights[base + 1 : base + 3] = 8
                self.weights[base + 14] = 8
        elif task == "jackal-stage1":
            self.weights[4:6] = [32, 8]
            self.weights[3] = 2
            self.weights[40:136] = 0.1
            self.weights[136:186] = 0.5
            for base in (8, 24):
                self.weights[base + 1 : base + 5] = [8, 8, 8, 16]
        elif task == "chipndale-zone0":
            self.weights[4] = 24
            self.weights[40:208] = 0.25
            self.weights[3] = self.weights[7] = 2
            for base in (8, 24):
                self.weights[base + 1 : base + 3] = [12, 12]
                self.weights[base + 5] = 12
                self.weights[base + 7] = 4
        elif task == "iceclimber-mountain1":
            self.weights[40:232] = 0.1
            for base in [8, 24]:
                self.weights[base + 6 : base + 10] = [16, 32, 64, 64]
                self.weights[base + 12 : base + 14] = 2
        elif task == "doubledragon2-mission1":
            self.weights[4] = 24
            self.weights[3] = self.weights[7] = 2
            for base in (8, 24):
                self.weights[base + 1 : base + 4] = [8, 8, 8]
                self.weights[base + 8] = 16
        elif task == "rivercity-moose":
            self.weights[4] = 34  # Do not borrow actions from a different room.
            for base in (8, 24):
                self.weights[base + 1 : base + 4] = [8, 8, 2]
                self.weights[base + 6 : base + 8] = [4, 8]
        self.xs = xs * self.weights
        self.ys = ys

    def label(self, observation):
        if self.combat_controls and round(observation[4] * 34) == 5:
            return np.asarray(teacher(self.task, observation, "punch-edges"), np.int64)
        if self.route_controls and round(observation[4] * 34) < 5:
            return np.asarray(teacher(self.task, observation), np.int64)
        diff = self.xs - np.asarray(observation, np.float32) * self.weights
        label = self.ys[np.einsum("ij,ij->i", diff, diff).argmin()].copy()
        if self.grounded:
            label[2] = int(observation[13] > 0.5 and observation[20] < 0.5)
            if observation[13] < 0.5:
                label[0] = {1: 2, 2: 1}.get(round(observation[16] * 255), 0)
        return label


def balanced_dataset(demos, recent, repeat, limit, rng):
    """Keep successful routes represented when failed rollouts are much longer."""
    dx, dy = demos
    if repeat < 1 or limit < 1:
        raise ValueError("demonstration repeat and rollout limit must be positive")
    rx, ry = recent
    if len(rx) > limit:
        indices = rng.choice(len(rx), size=limit, replace=False)
        rx, ry = rx[indices], ry[indices]
    return (np.concatenate([dx] * repeat + [rx]), np.concatenate([dy] * repeat + [ry]))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--task",
        choices=[k for k in CONTRACTS if k not in ("contra-jungle", "contra-base1")],
        required=True,
    )
    p.add_argument("--rom", required=True)
    p.add_argument("--replay", action="append", required=True)
    p.add_argument("--output", required=True)
    p.add_argument("--epochs", type=int, default=150)
    p.add_argument("--rounds", type=int, default=8)
    p.add_argument("--horizon", type=int, default=18000)
    p.add_argument("--seed", type=int, default=83)
    p.add_argument("--resume", help="continue a validated bundle with a fresh optimizer")
    p.add_argument(
        "--balance-directions",
        action="store_true",
        help="upweight rare horizontal/vertical correction labels",
    )
    p.add_argument("--demo-repeat", type=int, default=4)
    p.add_argument(
        "--grounded-controls",
        action="store_true",
        help="Ice Climber: use state-derived jump edges in the training teacher",
    )
    p.add_argument(
        "--route-controls",
        action="store_true",
        help="River City: label route recovery with the observation-only teacher during DAgger",
    )
    p.add_argument(
        "--combat-controls",
        action="store_true",
        help="River City DAgger: use observed punch edges during boss combat",
    )
    p.add_argument("--dagger-limit", type=int, default=8000)
    p.add_argument(
        "--condition-observations",
        action="store_true",
        help="rescale game-specific learning features and fold that scale into exported weights",
    )
    a = p.parse_args()
    if min(a.epochs, a.rounds, a.demo_repeat, a.dagger_limit) < 1 or not 1 <= a.horizon <= 108000:
        p.error("invalid training budget")
    raw = Path(a.rom).read_bytes()
    core = TaskEpisode(raw, a.task, a.horizon)
    datasets = [collect(core, a.task, json.loads(Path(path).read_text())) for path in a.replay]
    xs, ys = [np.concatenate([d[i] for d in datasets]) for i in range(2)]
    expert = NearestDemonstration(
        xs, ys, a.task, a.grounded_controls, a.route_controls, a.combat_controls
    )
    demos = (xs.copy(), ys.copy())
    rng = np.random.default_rng(a.seed)
    out = Path(a.output)
    out.mkdir(parents=True, exist_ok=False)
    (out / "configuration.json").write_text(
        json.dumps(
            dict(
                arguments=vars(a),
                trainer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                torch_version=torch.__version__,
                demonstration_sha256=[
                    hashlib.sha256(Path(path).read_bytes()).hexdigest() for path in a.replay
                ],
            ),
            indent=2,
        )
        + "\n"
    )
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
        # Bundles already contain the folded learning scale. Undo it once before
        # continuing optimization in the current learning coordinate system.
        with torch.no_grad():
            net.layers[0].weight.div_(torch.as_tensor(scale))
        resume_hash = manifest["weights_sha256"]
    opt = torch.optim.Adam(net.parameters(), lr=0.0005)
    bestkey = (-1, -1, -1, -1e9)
    best = None
    for rnd in range(a.rounds):
        direction_weights = direction_class_weights(ys) if a.balance_directions else None
        metrics = fit(net, xs * scale, ys, a.epochs, opt, direction_weights)
        directory = out / f"round-{rnd:02d}"
        export(
            runtime_actor(net, scale),
            directory,
            dict(
                name=a.task + " demonstration candidate",
                algorithm="verified controller demonstrations + nearest-demonstration DAgger",
                route_recovery_teacher=a.route_controls,
                punch_edge_combat_teacher=a.combat_controls,
                resume_weights_sha256=resume_hash,
                direction_class_weights=(
                    [w.tolist() for w in direction_weights]
                    if direction_weights is not None
                    else None
                ),
                round=rnd,
                examples=len(xs),
                epochs=a.epochs,
                seed=a.seed,
                fit=metrics,
                learning_feature_scale=scale.tolist(),
                runtime="fixed neural actor on the original structured observation; no search or teacher",
            ),
            xs,
            task=a.task,
        )
        actor = load_actor(directory)
        rows = []
        for warmup in [0, 16, 40, 80]:
            row, inputs, _, _ = rollout(core, a.task, warmup, actor)
            rows.append(row)
            write_replay(directory / f"selection-{warmup}.json", a.task, row, inputs)
        totals = summary(rows)
        report = dict(
            version=1,
            task=a.task,
            split="selection",
            model=json.loads((directory / "manifest.json").read_text()),
            summary=totals,
            episodes=rows,
        )
        (directory / "selection.json").write_text(json.dumps(report, indent=2) + "\n")
        key = (
            totals["clears"],
            totals["both_survive"],
            totals["mean_progress"],
            -totals["mean_deaths"],
        )
        if key > bestkey:
            bestkey, best = key, str(directory)
        (out / "best.json").write_text(json.dumps(dict(model=best, key=bestkey)) + "\n")
        print(json.dumps(dict(round=rnd, fit=metrics, selection=totals)), flush=True)
        if totals["clears"] == 4:
            break
        nx, ny = [], []
        for warmup in [0, (rnd * 17 + 8) % 97]:
            step = json.loads(core.reset(warmup))
            while True:
                masks = []
                for o in step["observations"]:
                    masks.append(actor.buttons(o) if o[1] > 0.5 else 0)
                    if o[1] > 0.5 and o[8] > 0.5 and o[12] < 0.5:
                        nx.append(o)
                        ny.append(expert.label(o))
                step = json.loads(core.step(masks))
                if step["terminated"] or step["truncated"]:
                    break
        if nx:
            xs, ys = balanced_dataset(
                demos,
                (np.asarray(nx, np.float32), np.asarray(ny, np.int64)),
                a.demo_repeat,
                a.dagger_limit,
                rng,
            )


if __name__ == "__main__":
    main()
