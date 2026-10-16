"""Stable Baselines adapter over the Rust worker pool.

SB3 autoreset is explicit here: terminal_observation and truncation are retained
in info before resetting exactly the finished row. The lower-level Batch API
never autoresets. No Python subprocesses or per-frame JSON are involved.
"""

from pathlib import Path
import gymnasium as gym
import numpy as np
from stable_baselines3.common.vec_env import VecEnv
from .transport import Batch
from .actions import FACTORS, buttons
from contra_training.env import scripted
from contra_training.tasks import teacher
from contra_training.teacher import action as jungle_route


def individual_outcome(port_rewards, terminated, truncated, info, slot):
    """Contra learner objective; never collect a partner's future after elimination.

    Keep native per-port progress/death accounting. A team clear only pays its
    +10 bonus when this learner is still active. This is a training terminal,
    not a change to the emulator's cooperative episode or deployed runtime.
    """
    active = bool(info["state"]["players"][slot]["active"])
    team_success = bool(info["success"])
    reward = float(port_rewards[slot]) - (10.0 if team_success and not active else 0.0)
    info["team_success"] = team_success
    info["success"] = team_success and active
    info["learner_eliminated"] = not active
    terminated = bool(terminated or not active)
    return reward, terminated, bool(truncated and not terminated)


class RustVecEnv(VecEnv):
    def __init__(
        self,
        rom,
        count=4,
        task="contra-jungle",
        player=0,
        horizon=7200,
        teammate="scripted",
        start=None,
        warmup_range=24,
        policy_partner=None,
        teacher_profile="standard",
        curriculum=None,
        skip_non_gameplay=False,
        observation_schema=None,
        contra_individual=False,
        contra_solo=False,
    ):
        if contra_solo and (
            task != "contra-jungle"
            or player != 0
            or teammate != "idle"
            or policy_partner is not None
        ):
            raise ValueError("solo Contra requires P1 with no teammate policy and teammate='idle'")
        if player not in (0, 1, "alternate") or teammate not in ("scripted", "route", "idle"):
            raise ValueError("invalid player or teammate")
        if teammate == "route" and task != "contra-jungle":
            raise ValueError("route teammate requires Contra Jungle")
        if contra_individual and task != "contra-jungle":
            raise ValueError("individual reward is defined only for Contra")
        self.contra_individual = contra_individual
        if not 1 <= warmup_range <= 241:
            raise ValueError("warmup range must be in 1..241")
        if task == "contra-jungle" and teacher_profile != "standard":
            raise ValueError("Contra uses the route/scripted teammate choice")
        if task != "contra-jungle":
            teacher(task, np.zeros(256), teacher_profile)
        self.teacher_profile = teacher_profile
        if curriculum and task != "iceclimber-mountain1":
            raise ValueError("replay curriculum currently supports Ice Climber only")
        self.curriculum = curriculum or []
        self.batch = Batch(
            Path(rom).read_bytes(),
            task,
            count,
            horizon,
            Path(start).read_bytes() if start else None,
            skip_non_gameplay=skip_non_gameplay,
            contra_solo=contra_solo,
        )
        self.task, self.player, self.teammate = task, player, teammate
        self.slots = np.arange(count) % 2 if player == "alternate" else np.full(count, player)
        self.warmup_range = warmup_range
        self.policy_partner = policy_partner
        self.rngs = [np.random.default_rng() for _ in range(count)]
        self.last = None
        self.actions = None
        self.observation_schema = observation_schema
        inputs = 256
        if observation_schema is not None:
            from contra_training.contracts import POLICY_VARIANTS

            variant = POLICY_VARIANTS[observation_schema]
            if variant["task"] != task or variant["assistance"]:
                raise ValueError("vector observation must be an autonomous policy for this task")
            inputs = variant["inputs"]
        super().__init__(
            count, gym.spaces.Box(-1, 1, (inputs,), np.float32), gym.spaces.MultiDiscrete(FACTORS)
        )

    def _policy_observations(self, observations):
        if self.observation_schema is None:
            return observations[np.arange(self.num_envs), self.slots].copy()
        return np.asarray(
            self.batch.core.policy_features(self.observation_schema, self.slots.tolist()),
            dtype=np.float32,
        )

    def reset(self):
        warmups = []
        for i, seed in enumerate(self._seeds):
            if seed is not None:
                self.rngs[i] = np.random.default_rng(seed)
            default = (
                int(seed % (32 if self.warmup_range == 24 else self.warmup_range))
                if seed is not None
                else int(self.rngs[i].integers(0, self.warmup_range))
            )
            warmups.append(int(self._options[i].get("warmup", default)))
        result = self.batch.reset(warmups)
        self.last = result[0]
        self.reset_infos = [s.info for s in result[4]]
        if self.curriculum:
            for i in range(self.num_envs):
                entry = self._curriculum_reset(i)
                if entry is not None:
                    self.last[i] = entry["observations"]
                    self.reset_infos[i] = entry["info"]
        self._reset_seeds()
        self._reset_options()
        self.actions = None
        return self._policy_observations(self.last)

    def _curriculum_reset(self, index):
        # Half the resets retain the ordinary beginning of the full task.
        if not self.curriculum or self.rngs[index].random() >= 0.5:
            return None
        entry = self.curriculum[int(self.rngs[index].integers(len(self.curriculum)))]
        step = self.batch.reset_with_prefix_at(index, entry["warmup"], entry["prefix"])
        step["info"]["curriculum_start_frame"] = len(entry["prefix"])
        return step

    def step_async(self, actions):
        if self.last is None or self.batch is None:
            raise RuntimeError("reset an open environment before stepping")
        if self.actions is not None:
            raise RuntimeError("a step is already pending")
        actions = np.asarray(actions)
        if actions.shape != (self.num_envs, 4):
            raise ValueError("one factorized action per environment is required")
        self.actions = [buttons(a) for a in actions]

    def step_wait(self):
        if self.actions is None:
            raise RuntimeError("call step_async first")
        masks = []
        for i, action in enumerate(self.actions):
            player = self.slots[i]
            teammate = 0
            obs = self.last[i, 1 - player]
            if self.policy_partner is not None and i % 4 >= 2:
                teammate = self.policy_partner.buttons(obs)
            elif self.teammate == "route":
                teammate = buttons(jungle_route(obs))
            elif self.teammate == "scripted":
                teammate = buttons(
                    scripted(obs)
                    if self.task == "contra-jungle"
                    else teacher(self.task, obs, self.teacher_profile)
                )
            pair = [0, 0]
            pair[player], pair[1 - player] = action, teammate
            masks.append(pair)
        self.actions = None
        obs, rewards, term, trunc, rows = self.batch.step(masks)
        infos = [dict(s.info, skipped_transition_frames=s.skipped_transition_frames) for s in rows]
        if self.contra_individual:
            for i, row in enumerate(rows):
                rewards[i], term[i], trunc[i] = individual_outcome(
                    row.rewards, term[i], trunc[i], infos[i], int(self.slots[i])
                )
        done = term | trunc
        policy_obs = self._policy_observations(obs)
        for i in range(self.num_envs):
            infos[i]["TimeLimit.truncated"] = bool(trunc[i] and not term[i])
            if done[i]:
                infos[i]["terminal_observation"] = policy_obs[i].copy()
                reset = self.batch.reset_at(i, int(self.rngs[i].integers(0, self.warmup_range)))
                reset = self._curriculum_reset(i) or reset
                obs[i] = reset["observations"]
                self.reset_infos[i] = reset["info"]
        self.last = obs
        if done.any():
            policy_obs[done] = self._policy_observations(obs)[done]
        return policy_obs, rewards, done, infos

    def close(self):
        self.batch = None
        self.last = None
        self.actions = None

    def get_attr(self, name, indices=None):
        if name == "player":
            return [int(self.slots[i]) for i in self._get_indices(indices)]
        values = {
            "render_mode": None,
            "player": self.player,
            "task": self.task,
            "teammate": self.teammate,
        }
        if name not in values:
            raise AttributeError(name)
        return [values[name] for _ in self._get_indices(indices)]

    def set_attr(self, name, value, indices=None):
        raise AttributeError("Training contracts are fixed; construct a new environment")

    def env_method(self, name, *args, indices=None, **kwargs):
        raise AttributeError(name)

    def env_is_wrapped(self, wrapper_class, indices=None):
        return [False for _ in self._get_indices(indices)]
