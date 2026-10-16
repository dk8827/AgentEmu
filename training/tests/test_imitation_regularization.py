import unittest

import numpy as np

try:
    import torch
except ImportError:
    torch = None


@unittest.skipIf(torch is None, "requires training dependencies")
class ImitationRegularization(unittest.TestCase):
    def test_smoothed_objective_reduces_excessive_confidence(self):
        from contra_training.improve import fit

        torch.set_num_threads(1)
        # A one-input actor already predicts every label correctly with a large
        # margin. Smoothing should lower its confidence instead of increasing it.
        actor = torch.nn.Linear(1, 10)
        with torch.no_grad():
            actor.weight.zero_()
            actor.bias.zero_()
            actor.bias[[0, 3, 6, 8]] = 20
        before = actor.bias.detach().clone()
        fit(
            actor,
            np.zeros((8, 1), np.float32),
            np.zeros((8, 4), np.int64),
            1,
            torch.optim.SGD(actor.parameters(), lr=0.1),
            label_smoothing=0.02,
        )
        self.assertTrue(torch.all(actor.bias[[0, 3, 6, 8]] < before[[0, 3, 6, 8]]))
        for invalid in (-0.1, 1, float("nan"), float("inf")):
            with self.assertRaises(ValueError):
                fit(actor, None, None, 1, None, label_smoothing=invalid)
