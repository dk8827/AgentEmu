from pathlib import Path
import gymnasium as gym
import numpy as np
from .typed import unpack
from ._native import Contra

from agentemu_training.actions import FACTORS, buttons


def scripted(observation):
    """Simple disclosed baseline: advance, jump from ground, pulse fire. No hidden state."""
    return np.array([2, 0, int(observation[13] > 0.5), int(observation[3] < 0.5)], dtype=np.int64)


class ContraEnv(gym.Env):
    """One learned port and scripted/idle teammate; one emulated frame per step."""

    metadata = {"render_modes": []}

    def __init__(self, rom, player=0, horizon=7200, teammate="scripted", single_player=False):
        if single_player and (player != 0 or teammate != "idle"):
            raise ValueError("solo Contra requires player=0 and teammate='idle'")
        if player not in (0, 1) or teammate not in ("scripted", "idle"):
            raise ValueError("invalid player or teammate")
        self.core = Contra(Path(rom).read_bytes(), horizon, single_player)
        self.single_player = single_player
        self.player, self.teammate = player, teammate
        self.observation_space = gym.spaces.Box(-1, 1, shape=(256,), dtype=np.float32)
        self.action_space = gym.spaces.MultiDiscrete(FACTORS)
        self.last = None

    def reset(self, *, seed=None, options=None):
        super().reset(seed=seed)
        # Every explicit seed replays exactly; unspecified resets draw from Gym's seeded RNG.
        start = int(seed) if seed is not None else int(self.np_random.integers(0, 24))
        self.last = unpack(self.core.reset_data(start))
        return np.asarray(self.last["observations"][self.player], dtype=np.float32), self.last[
            "info"
        ]

    def step(self, action):
        if self.last is None:
            raise RuntimeError("reset before step")
        masks = [0, 0]
        action_mask = buttons(action)
        masks[self.player] = action_mask if self.last["info"]["state"]["gameplay"] else 0
        if self.teammate == "scripted" and self.last["info"]["state"]["gameplay"]:
            masks[1 - self.player] = buttons(scripted(self.last["observations"][1 - self.player]))
        self.last = unpack(self.core.step_data(masks))
        r = self.last
        # Solo uses P1's reward; co-op preserves the shared mean of both ports.
        return (
            np.asarray(r["observations"][self.player], dtype=np.float32),
            float(r["rewards"][0] if self.single_player else np.mean(r["rewards"])),
            r["terminated"],
            r["truncated"],
            r["info"],
        )
