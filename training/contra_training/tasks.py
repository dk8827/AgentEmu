"""Gym environments and observation-only teachers for versioned expansion tasks."""

from pathlib import Path
import gymnasium as gym
import numpy as np
from .typed import unpack
from ._native import TaskEpisode
from .env import FACTORS, buttons


def teacher(task, o, profile="standard"):
    profiles = {
        "punch-edges": "rivercity-moose",
        "walk-and-punch": "rivercity-moose",
        "lateral-jumps": "doubledragon2-mission1",
        "dd2-punch": "doubledragon2-mission1",
        "snow-platforms": "snowbros-floor1",
        "tmnt-fast-jump": "tmnt2-scene1",
        "tmnt-bit-jump": "tmnt2-scene1",
        "tmnt-nearest-bit": "tmnt2-scene1",
        "tmnt-pixel-bit": "tmnt2-scene1",
    }
    if profile != "standard" and profiles.get(profile) != task:
        raise ValueError("teacher profile is not supported for this task")
    if task == "lifeforce-stage1":
        # Simple terrain-aware baseline; complete route is a separate demonstration.
        if o[13] < 0.5:
            return [0, 0, 0, 0]
        x, y = o[9] * 255, o[10] * 239
        blocked = o[173] > 0.5
        vertical = (1 if y > 120 else 2) if blocked else (1 if y > 122 else 2 if y < 118 else 0)
        upgrade = round(o[21] * 6)
        buy = (upgrade == 1 and o[16] * 8 < 2) or upgrade in (2, 3, 5, 6)
        return [
            2 if x < 62 else 1 if x > 66 else 0,
            vertical,
            int(buy and o[7] > 0.5),
            1,
        ]
    if task == "jackal-stage1":
        # Exploratory terrain-aware baseline, not the verified demonstration.
        if o[8] < 0.5:
            return [0, 0, 0, 0]
        blocked = o[143] > 0.5  # Eight pixels north in the 5x5 collision grid.
        return [2 if blocked else 0, 0 if blocked else 1, int(o[3] > 0.5), 1]
    if task == "chipndale-zone0":
        # Exploratory route baseline; the verified clear is a separate replay.
        if o[8] < 0.5:
            return [0, 0, 0, 0]
        if round(o[4] * 24) == 24:
            x = o[9] * 2048
            throw = not (o[15] > 0.5 and o[19] > 0.5)
            return [
                2 if x < 1149 else 1 if x > 1153 else 0,
                1,
                int(o[7] > 0.5),
                int(throw),
            ]
        return [2, 0, int(o[7] > 0.5), int(o[3] > 0.5)]
    if task == "worldcup-firstmatch":
        x, depth = round(o[9] * 1024), round(o[10] * 512)
        dx, dy = round(o[11] * 1024), round(o[12] * 512)
        near = abs(dx) < 30 and abs(dy) < 20
        direction = 1 if o[4] > 0.5 else -1
        if near:
            dx, dy = direction * 80, 224 - depth
        shoot = (x > 650 if direction == 1 else x < 370) and abs(depth - 224) < 60
        return [
            2 if dx > 4 else 1 if dx < -4 else 0,
            1 if dy > 4 else 2 if dy < -4 else 0,
            int(o[7] > 0.5 and (not near or o[5] > 0.5)),
            int(o[3] > 0.5 and shoot),
        ]
    if task == "doubledragon2-mission1":
        stage = round(o[4] * 24)
        x, depth = o[9] * 1024, o[10] * 256
        if o[19] > 0.5:
            return [0, 1, 0, 0]
        enemies = [o[40 + i * 12 : 52 + i * 12] for i in range(2) if o[40 + i * 12] > 0.5]
        if enemies:
            dx, dy = enemies[0][1] * 1024, enemies[0][2] * 256
            h = 2 if dx > 16 else 1 if dx < -16 else 0
            v = 1 if dy > 2 else 2 if dy < -2 else 0
            a = b = int(o[7] > 0.5)
            if not a and abs(dx) < 48 and o[3] > 0.5:
                a, b = int(dx >= 0), int(dx < 0)
            if profile == "dd2-punch":
                # A strikes right and B strikes left. Avoid spending each
                # close-range attack window jumping over the opponent.
                a = int(dx >= 0 and o[3] > 0.5 and abs(dx) < 48)
                b = int(dx < 0 and o[3] > 0.5 and abs(dx) < 48)
        else:
            target = (
                712
                if 8 <= stage <= 10
                else (
                    660
                    if stage == 11
                    else (
                        810
                        if stage == 12
                        else (920 if stage in (13, 14) else 648 if stage in (15, 16, 17) else None)
                    )
                )
            )
            h = 2 if target is None or x < target - 2 else 1 if x > target + 2 else 0
            v = 1
            a = b = 0
            if stage == 11:
                v = 1 if depth < 158 else 2 if depth > 162 else 0
            if stage == 12:
                v = 1 if depth < 128 else 2 if depth > 132 else 0
                a = b = int(o[7] > 0.5)
            if stage < 8:
                a = b = int(o[7] > 0.5)
                if profile in ("lateral-jumps", "dd2-punch"):
                    v = 0
            if stage in (13, 14) and h and depth > 130:
                v = 2
            if stage == 17 and h and depth > 160:
                v = 2
        if 11 <= stage <= 12 and h and depth > 132 and x > 696:
            v, a, b = 2, 0, 0
        return [h, v, a, b]
    if task == "tmnt2-scene1":
        # The adapter packs enemies in nearest-first order. The compact profile
        # deliberately ignores later entries, including impossible sparse inputs.
        compact = profile in ("tmnt-nearest-bit", "tmnt-pixel-bit")
        count = 1 if compact else 11
        enemies = [o[40 + i * 12 : 52 + i * 12] for i in range(count) if o[40 + i * 12] > 0.5]
        if not enemies:
            return [2, 0, 0, 0]
        e = enemies[0]
        dx, dy = e[1] * 255, e[2] * 255
        if compact:
            dx, dy = round(float(dx)), round(float(dy))
        if profile in ("tmnt-fast-jump", "tmnt-bit-jump", "tmnt-nearest-bit", "tmnt-pixel-bit"):
            near = abs(dx) < 48 and abs(dy) < 16
            # The older JSON teacher moved at exactly 24/3 pixels because its
            # normalized float32 fields became slightly larger Python doubles.
            # NumPy float32 multiplication rounded back to 24/3 while fitting.
            # Specify those successful rollout boundaries in pixels explicitly.
            horizontal_limit = 23 if profile == "tmnt-pixel-bit" else 24
            vertical_limit = 2 if profile == "tmnt-pixel-bit" else 3
            horizontal = 2 if dx > horizontal_limit else 1 if dx < -horizontal_limit else 0
            if (dx < 0) != (o[17] > 0.5):
                horizontal = 1 if dx < 0 else 2
            jump = near and (
                o[3] < 0.5 if profile != "tmnt-fast-jump" else round(o[2] * 255) % 16 == 0
            )
            return [
                horizontal,
                2 if dy > vertical_limit else 1 if dy < -vertical_limit else 0,
                int(jump),
                int(near and not jump and o[3] > 0.5),
            ]
        near = abs(dx) <= 48
        jump = near and o[7] > 0.5
        attack = near and not jump and o[3] > 0.5
        return [
            2 if dx > 24 else 1 if dx < -24 else 0,
            2 if dy > 3 else 1 if dy < -3 else 0,
            int(jump),
            int(attack),
        ]
    if task == "rivercity-moose":
        # Observation-only comparison baseline; offline search demonstrations
        # remain separate from this deliberately simple controller.
        room, x, depth = round(o[4] * 34), o[9] * 1024, o[10] * 255
        clock = round(o[2] * 255)
        if room < 5:
            lane = 80 if o[0] < 0.5 else 104
            h = 0 if profile != "walk-and-punch" and clock % 120 in (2, 3) else 2
            v = 1 if depth < lane - 2 else 2 if depth > lane + 2 else 0
            if room == 1 and x > 435:
                h = 2 if x < 461 else 1 if x > 467 else 0
                v = 1
            jump = int(profile != "walk-and-punch" and clock % 60 < 2)
            return [h, v, jump, jump]
        enemies = [o[40 + i * 16 : 56 + i * 16] for i in range(6)]
        enemies = [e for e in enemies if e[0] > 0.5 and e[6] > 0.5]
        if not enemies:
            return [0, 0, 0, 0]
        e = min(enemies, key=lambda e: abs(e[1]))
        dx, dz = e[1] * 1024, e[2] * 255
        h = 2 if dx > 0 else 1
        if abs(dz) > 4 and abs(dx) < 16:
            h = 0
        attack = int(
            abs(dx) <= 42
            and (o[20] < 0.5 if profile in ("punch-edges", "walk-and-punch") else clock % 12 < 3)
        )
        return [h, 1 if dz > 0.5 else 2 if dz < -0.5 else 0, attack, 0]
    if task == "iceclimber-mountain1":
        # Experimental terrain baseline; it is not the verified route-search teacher.
        x, floor = o[9] * 255, round(o[14] * 255)
        cells = o[40 + min(7, floor) * 24 : 64 + min(7, floor) * 24]
        holes = [i for i in range(23) if cells[i] < 0 and cells[i + 1] < 0]
        target = (
            min((32 + i * 8 for i in holes), key=lambda tx: abs(tx - x))
            if holes
            else (88 if o[0] < 0.5 else 152)
        )
        dx = target - x
        direction = 2 if dx > 3 else 1 if dx < -3 else (2 if o[0] < 0.5 else 1) if holes else 0
        if o[18] > 0.5:
            motion = round(o[16] * 255)
            direction = 2 if motion == 1 else 1 if motion == 2 else 0
        return [direction, 0, int(o[13] > 0.5 and o[20] < 0.5), 0]
    if task == "bubblebobble-round1":
        entities = [o[40 + i * 8 : 48 + i * 8] for i in range(7) if o[40 + i * 8] > 0.5]
        dx = entities[0][1] * 256 if entities else 128 - o[9] * 255
        return [
            2 if dx > 4 else 1 if dx < -4 else 0,
            0,
            int(o[7] > 0.5),
            int(o[3] < 0.5),
        ]
    if task == "snowbros-floor1":
        candidates = [o[40 + i * 7 : 47 + i * 7] for i in range(8)]
        candidates = [e for e in candidates if e[0] > 0.5 and 2 <= round(e[5] * 63) <= 18]
        target = (
            min(candidates, key=lambda e: abs(e[1] * 256) + abs(e[2] * 240))
            if candidates
            else [0, 0, 0, 0, 0, 0, 0]
        )
        dx, dy = target[1] * 256, target[2] * 240
        if profile == "snow-platforms":
            horizontal = 2 if dx > 1 else 1 if dx < -1 else 0
            jump = o[13] > 0.5 and (dy < -8 or abs(dx) < 24)
            if dy < -16:
                # Row 1 is sixteen pixels above the player. Find a landing
                # platform before jumping, then avoid drifting into its gap.
                cells = [i for i in range(8) if o[96 + (8 + i) * 4 + 1] > 0.5]
                if cells:
                    column = min(cells, key=lambda i: abs(i - 2))
                    horizontal = 2 if column > 2 else 1 if column < 2 else 0
                    jump = o[13] > 0.5 and column == 2
                if o[13] < 0.5:
                    horizontal = 0
            return [horizontal, 0, int(jump), int(o[3] < 0.5)]
        return [
            2 if dx > 5 else 1 if dx < -5 else 0,
            0,
            int(o[13] > 0.5 and (dy < -8 or abs(dx) < 24)),
            int(o[3] < 0.5),
        ]
    if task == "contra-base1":
        if o[5] > 0.5:
            return [0, 1, 0, 0]
        candidates = [o[48 + i * 13 : 61 + i * 13] for i in range(16)]
        cores = [e for e in candidates if e[0] > 0.5 and e[5] > 0 and (e[6] > 0.5 or e[11] > 0.5)]
        x = o[17] * 255
        target = min(cores, key=lambda e: abs(e[1])) if cores else None
        tx = target[1] * 256 + x if target is not None else 128
        crouch = target is not None and target[12] > 0.5
        if o[7] < 0.5:
            tx = 128 + (tx - 128) * (3.2 if crouch else 8 / 3)
        dx = tx - x
        danger = any(
            e[0] > 0.5 and e[8] > 0.5 and abs(e[1] * 256) < 14 and -40 < e[2] * 240 < 14
            for e in candidates
        )
        roller = any(
            e[0] > 0.5 and (e[9] > 0.5 or e[10] > 0.5) and abs(e[1] * 256) < 24 and e[2] * 240 > -28
            for e in candidates
        )
        vertical = 1 if o[7] > 0.5 and abs(dx) < 4 else 2 if crouch or danger else 0
        return [
            2 if dx > 2 else 1 if dx < -2 else 0,
            vertical,
            int(roller and o[21] > 0.5),
            int(o[3] < 0.5),
        ]
    raise ValueError("unknown task")


class TaskEnv(gym.Env):
    metadata = {"render_modes": []}

    def __init__(self, rom, task, player=0, horizon=12000, start=None, single_player=False):
        if player not in (0, 1):
            raise ValueError("player must be 0 or 1")
        if single_player and player != 0:
            raise ValueError("solo training requires player 0")
        self.single_player = single_player
        self.task, self.player = task, player
        self.core = TaskEpisode(
            Path(rom).read_bytes(),
            task,
            horizon,
            Path(start).read_bytes() if start else None,
            single_player,
        )
        self.observation_space = gym.spaces.Box(-1, 1, (256,), np.float32)
        self.action_space = gym.spaces.MultiDiscrete(FACTORS)
        self.last = None

    def reset(self, *, seed=None, options=None):
        super().reset(seed=seed)
        warmup = int(
            (options or {}).get(
                "warmup",
                int(self.np_random.integers(0, 97)) if seed is None else seed % 97,
            )
        )
        self.last = unpack(self.core.reset_data(warmup))
        return (
            np.asarray(self.last["observations"][self.player], np.float32),
            self.last["info"],
        )

    def step(self, action):
        if self.last is None:
            raise RuntimeError("reset before step")
        masks = [0, 0]
        masks[self.player] = buttons(action)
        if not self.single_player:
            masks[1 - self.player] = buttons(
                teacher(self.task, self.last["observations"][1 - self.player])
            )
        self.last = unpack(self.core.step_data(masks))
        r = self.last
        return (
            np.asarray(r["observations"][self.player], np.float32),
            r["reward"],
            r["terminated"],
            r["truncated"],
            r["info"],
        )
