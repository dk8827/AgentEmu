import unittest

try:
    import torch
except ImportError:
    torch = None


@unittest.skipIf(torch is None, "requires training dependencies")
class ExplorationCalibration(unittest.TestCase):
    def test_scaling_retains_actions_but_allows_stochastic_alternatives(self):
        from contra_training.model import ActorNet, calibrate_action_logits

        torch.set_num_threads(1)
        original = ActorNet()
        with torch.no_grad():
            original.layers[-1].weight.zero_()
            original.layers[-1].bias.copy_(
                torch.tensor([0.0, 20.0, -20.0, 20.0, 0.0, -20.0, 0.0, 20.0, 20.0, 0.0])
            )
        before = {k: v.clone() for k, v in original.state_dict().items()}
        scaled = calibrate_action_logits(original, 0.02)
        with torch.no_grad():
            x = torch.zeros((4, 256))
            logits, reduced = original(x), scaled(x)
            torch.testing.assert_close(reduced, logits * 0.02)
            for a, b in zip(logits.split((3, 3, 2, 2), 1), reduced.split((3, 3, 2, 2), 1)):
                torch.testing.assert_close(a.argmax(1), b.argmax(1))
                self.assertTrue(
                    torch.all(
                        torch.distributions.Categorical(logits=b).entropy()
                        > torch.distributions.Categorical(logits=a).entropy()
                    )
                )
        for name, value in original.state_dict().items():
            torch.testing.assert_close(value, before[name], rtol=0, atol=0)
            if not name.startswith("layers.2."):
                torch.testing.assert_close(scaled.state_dict()[name], value, rtol=0, atol=0)
        for invalid in (0, -1, 1.1, float("nan")):
            with self.assertRaises(ValueError):
                calibrate_action_logits(original, invalid)
