"""Typed training transport. Observation buffers own their bytes and remain valid
across future steps; NumPy views are read-only and never alias emulator memory.
"""

import numpy as np


def unpack(step):
    return {
        "observations": np.frombuffer(step.observations, dtype="<f4").reshape(2, 256),
        "rewards": step.rewards,
        "reward": step.reward,
        "terminated": step.terminated,
        "truncated": step.truncated,
        # Scalar transports never skip frames; older implementations omit this
        # optional batch-training counter.
        "skipped_transition_frames": getattr(step, "skipped_transition_frames", 0),
        "info": step.info,
    }


class Batch:
    """Independent episodes in one Rust call; explicit terminal/reset semantics.

    Actions have shape (environments, players), containing NES button masks.
    Use reset_at after a terminal step; terminal observations are preserved.
    This is intentionally separate from Gym VectorEnv's autoreset conventions.
    """

    def __init__(
        self,
        rom,
        task,
        count,
        horizon=12000,
        start=None,
        skip_non_gameplay=False,
        contra_solo=False,
    ):
        from contra_training._native import EpisodeBatch

        self.core = EpisodeBatch(rom, task, count, horizon, start, skip_non_gameplay, contra_solo)
        self.count = count

    @staticmethod
    def _pack(steps):
        return (
            np.stack([np.frombuffer(s.observations, dtype="<f4").reshape(2, 256) for s in steps]),
            np.asarray([s.reward for s in steps], dtype=np.float32),
            np.asarray([s.terminated for s in steps], dtype=bool),
            np.asarray([s.truncated for s in steps], dtype=bool),
            steps,  # detailed info stays lazy until the consumer asks for it
        )

    def reset(self, warmups):
        return self._pack(self.core.reset(list(warmups)))

    def step(self, masks):
        return self._pack(self.core.step(np.asarray(masks).tolist()))

    def reset_at(self, index, warmup=0):
        return unpack(self.core.reset_at(index, warmup))

    def reset_with_prefix_at(self, index, warmup, prefix):
        return unpack(self.core.reset_with_prefix_at(index, warmup, prefix))
