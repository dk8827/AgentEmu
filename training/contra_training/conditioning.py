"""Training-only feature conditioning, folded into ordinary fixed policy weights."""

import copy
import numpy as np
import torch


def contra_teacher_scale(profile="standard"):
    """Keep the supervisor's inputs; fold omitted nuisance inputs to zero weights."""
    from .teacher import FEATURES

    names = {
        "header.camera_x",
        "header.world_x",
        "header.fire_phase",
        "players[0].grounded",
        "players[0].x",
        "players[0].y",
        "players[1].active",
        "players[1].dead",
        "players[1].grounded",
        "players[1].x",
    }
    names.update(
        f"entities[{i}].{field}" for i in range(8) for field in ("present", "dx", "dy", "kind0")
    )
    names.update(
        f"terrain[{row * 8 + col}].{field}"
        for row, col in ((3, 3), (3, 2), (4, 2), (2, 3), (1, 3))
        for field in ("floor", "solid")
    )
    if profile in ("contra-raised-platforms", "contra-island-takeoff"):
        names.update(
            f"terrain[{row * 8 + col}].{field}"
            for row, col in ((0, 3), (0, 4), (1, 4))
            for field in ("floor", "solid")
        )
    scale = np.zeros(256, np.float32)
    for name in names:
        scale[FEATURES[name]] = 4 if name.endswith((".dx", ".dy", ".camera_x", ".world_x")) else 1
    return scale


def teacher_feature_scale(task, profile="standard"):
    if task == "contra-jungle":
        return contra_teacher_scale(profile)
    if task == "tmnt2-scene1" and profile in (
        "tmnt-fast-jump",
        "tmnt-bit-jump",
        "tmnt-nearest-bit",
        "tmnt-pixel-bit",
    ):
        scale = np.zeros(256, np.float32)
        scale[3] = scale[17] = 1  # Existing pulse bit and facing direction.
        if profile == "tmnt-fast-jump":
            scale[2] = 16
        for base in range(40, 52 if profile in ("tmnt-nearest-bit", "tmnt-pixel-bit") else 172, 12):
            scale[base : base + 3] = [1, 4, 4]  # Presence, relative X and ground lane.
        return scale
    raise ValueError("feature selection requires a supported task and teacher profile")


def feature_scale(task):
    """Precondition small categorical fields during learning, not at runtime."""
    scale = np.ones(256, np.float32)
    if task == "lifeforce-stage1":
        scale[40:160] = 0.25
        for base in (8, 24):
            scale[base + 1 : base + 3] = 4
            scale[base + 7 : base + 9] = 8
    elif task == "jackal-stage1":
        scale[4:6] = [16, 4]
        scale[40:136] = 0.25
        for base in (8, 24):
            scale[base + 1 : base + 5] = [4, 4, 4, 8]
    elif task == "chipndale-zone0":
        scale[4] = 24
        for base in (8, 24):
            scale[base + 1 : base + 3] = [8, 4]
            scale[base + 5] = 12
        scale[40:208] = 0.25
    elif task == "worldcup-firstmatch":
        for base in (8, 24):
            scale[base + 1 : base + 5] = [4, 4, 8, 8]
        scale[48:72] = 4
    elif task == "iceclimber-mountain1":
        scale[5:7] = 32
        scale[40:232] = 0.25
        for base in (8, 24):
            scale[base + 3] = 32  # Lives.
            scale[base + 6 : base + 8] = 32  # Floor and motion.
            scale[base + 8 : base + 10] = 128  # Momentum and facing.
    elif task == "doubledragon2-mission1":
        scale[4] = 8  # Mission substages distinguish ladder/platform routes.
        for base in (8, 24):
            scale[base + 1 : base + 4] = [4, 4, 4]
            scale[base + 8] = 16  # Motion state.
    elif task == "rivercity-moose":
        scale[2] = 32  # Resolve narrow periodic clock intervals while learning.
        scale[4] = 8
        for base in (8, 24):
            scale[base + 1 : base + 4] = [4, 4, 2]
            scale[base + 6 : base + 8] = [4, 4]
        for base in range(40, 136, 16):
            scale[base + 1 : base + 5] = [16, 8, 4, 2]
            scale[base + 11 : base + 13] = 4
    return scale


def runtime_actor(actor, scale):
    """Fold the learning scale into weights; preserve the frozen observation ABI."""
    result = copy.deepcopy(actor)
    with torch.no_grad():
        result.layers[0].weight.mul_(torch.as_tensor(scale))
    return result


def direction_class_weights(labels):
    """Give rare route corrections weight without making absent classes infinite."""
    result = []
    for i in range(2):
        counts = np.bincount(labels[:, i], minlength=3).astype(np.float32)
        weights = np.sqrt(counts.max() / np.maximum(counts, 1)).clip(1, 12)
        weights[counts == 0] = 1
        result.append(torch.from_numpy(weights))
    return result


def unique_supervision(xs, ys, scale):
    """Remove exact repeats after feature selection; keep conflicting labels visible."""
    keys = np.concatenate((xs[:, scale != 0], ys.astype(np.float32)), axis=1)
    _, indices = np.unique(keys, axis=0, return_index=True)
    indices.sort()
    return xs[indices], ys[indices]
