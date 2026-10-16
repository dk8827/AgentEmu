import unittest
import json
import tempfile
from pathlib import Path
import torch
import numpy as np
from contra_training.model import ActorNet, export, validate_logits
from contra_training.contracts import CONTRACTS, ASSIST_CONTRACTS


class ExportParityTests(unittest.TestCase):
    def test_cancellation_error_with_identical_decisions(self):
        expected = np.array([[28.0, -418.0, 44.0, 50.0, -90.0, 0.26528, 3.0, -2.0, 2.0, -1.0]])
        actual = expected.copy()
        actual[0, 5] -= 0.0000575
        self.assertLess(validate_logits(actual, expected), 1e-6)

    def test_large_other_head_cannot_hide_error(self):
        expected = np.array([[10000.0, -10000.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0]])
        actual = expected.copy()
        actual[0, 4] += 0.001
        with self.assertRaises(AssertionError):
            validate_logits(actual, expected)

    def test_tiny_decision_change_is_rejected(self):
        expected = np.array([[1.0, 1.000001, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0]])
        actual = expected.copy()
        actual[0, 0] += 0.000002
        with self.assertRaises(AssertionError):
            validate_logits(actual, expected)

    def test_nonfinite_and_empty_are_rejected(self):
        for data in [np.zeros((0, 10)), np.full((1, 10), np.nan)]:
            with self.assertRaises(ValueError):
                validate_logits(data, data)


class ExportSchemaTests(unittest.TestCase):
    def test_explicit_canonical_schema_matches_default(self):
        torch.manual_seed(7)
        for assistance, contracts, inputs in [
            (False, CONTRACTS, 256),
            (True, ASSIST_CONTRACTS, 268),
        ]:
            with self.subTest(assistance=assistance), tempfile.TemporaryDirectory() as temp:
                actor = ActorNet(inputs)
                observations = np.random.default_rng(7).normal(size=(8, inputs)).astype(np.float32)
                root = Path(temp)
                export(actor, root / "default", {}, observations, assistance=assistance)
                export(
                    actor,
                    root / "explicit",
                    {},
                    observations,
                    assistance=assistance,
                    schema=contracts["contra-jungle"][1],
                )
                for name in ("manifest.json", "weights.safetensors", "parity.json"):
                    self.assertEqual(
                        (root / "default" / name).read_bytes(),
                        (root / "explicit" / name).read_bytes(),
                    )

    def test_history_variant_still_passes_native_parity(self):
        torch.manual_seed(7)
        with tempfile.TemporaryDirectory() as temp:
            output = Path(temp) / "model"
            export(
                ActorNet(272),
                output,
                {},
                np.zeros((8, 272), dtype=np.float32),
                schema="contra-jungle-272-history-v1",
            )
            self.assertEqual(
                json.loads((output / "manifest.json").read_text())["observation"],
                "contra-jungle-272-history-v1",
            )
            self.assertEqual(
                json.loads((output / "parity.json").read_text())["controller_decisions"], "exact"
            )

    def test_unknown_or_mismatched_schema_fails_before_writing(self):
        for schema, kwargs in [
            ("unknown", {}),
            ("contra-jungle-272-history-v1", {"task": "contra-base1"}),
            ("contra-jungle-272-history-v1", {"assistance": True}),
            ("contra-jungle-256-v1", {"assistance": True}),
        ]:
            with self.subTest(schema=schema, kwargs=kwargs), tempfile.TemporaryDirectory() as temp:
                output = Path(temp) / "model"
                with self.assertRaises(ValueError):
                    export(ActorNet(), output, {}, np.zeros((8, 256)), schema=schema, **kwargs)
                self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
