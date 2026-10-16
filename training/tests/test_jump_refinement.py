import unittest
import numpy as np
import torch
import gymnasium as gym
from stable_baselines3 import PPO

from contra_training.model import freeze_except_jump


class JumpPractice(gym.Env):
    observation_space = gym.spaces.Box(-1, 1, (256,), np.float32)
    action_space = gym.spaces.MultiDiscrete([3, 3, 2, 2])

    def reset(self, seed=None, options=None):
        super().reset(seed=seed)
        self.frame = 0
        return np.zeros(256, np.float32), {}

    def step(self, action):
        self.frame += 1
        return np.zeros(256, np.float32), float(action[2]), self.frame == 8, False, {}


class JumpRefinement(unittest.TestCase):
    def test_actual_ppo_updates_only_jump_actor_rows_and_value_network(self):
        torch.set_num_threads(1)
        policy = PPO(
            "MlpPolicy",
            JumpPractice(),
            seed=491,
            n_steps=16,
            batch_size=16,
            n_epochs=2,
            device="cpu",
            learning_rate=0.001,
            policy_kwargs=dict(
                net_arch=dict(pi=[64, 64], vf=[64, 64]), activation_fn=torch.nn.ReLU
            ),
        )
        freeze_except_jump(policy)
        before = {k: v.detach().clone() for k, v in policy.policy.state_dict().items()}
        policy.learn(128)
        after = policy.policy.state_dict()
        for k in before:
            if k.startswith("mlp_extractor.policy_net"):
                torch.testing.assert_close(after[k], before[k], rtol=0, atol=0)
        for k in ("action_net.weight", "action_net.bias"):
            torch.testing.assert_close(after[k][:6], before[k][:6], rtol=0, atol=0)
            torch.testing.assert_close(after[k][8:], before[k][8:], rtol=0, atol=0)
        self.assertFalse(torch.equal(before["action_net.bias"][6:8], after["action_net.bias"][6:8]))
        self.assertFalse(torch.equal(before["value_net.bias"], after["value_net.bias"]))
