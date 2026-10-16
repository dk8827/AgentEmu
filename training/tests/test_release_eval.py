import json
import unittest
import tempfile
import hashlib
from pathlib import Path
from contra_training.release_eval import acceptance, read_resume


def row(profile, success=True, deaths=0, frames=100):
    return dict(
        profile=profile, success=success, deaths=[deaths, 0], survivors=[True, True], frames=frames
    )


class ReleaseAcceptance(unittest.TestCase):
    def test_resume_requires_exact_case_prefix_and_unmodified_replays(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            manifest = dict(weights_sha256="weights", rom_sha256="rom")
            suite = dict(profiles=["pair"], baselines=["idle"], selection=[0, 16], seed=7)
            case = dict(
                row("pair", frames=1),
                warmup=0,
                seed=7,
                replay="pair-0.json",
                input_sha256=hashlib.sha256(bytes([0, 0])).hexdigest(),
            )
            replay = dict(rom_sha256="rom", result=case, inputs=[[0, 0]])
            report = dict(
                task="task",
                split="selection",
                model=manifest,
                suite_sha256="suite",
                scripted_partner="standard",
                episodes=[case],
                baselines=[],
            )
            (root / case["replay"]).write_text(json.dumps(replay))
            path = root / "report.json"
            path.write_text(json.dumps(report))
            args = (path, "task", "selection", manifest, suite, "suite", "standard")
            self.assertEqual(read_resume(*args), report)
            with self.assertRaisesRegex(ValueError, "protocol"):
                read_resume(path, "task", "selection", manifest, suite, "changed", "standard")
            replay["inputs"][0][0] = 1
            (root / case["replay"]).write_text(json.dumps(replay))
            with self.assertRaisesRegex(ValueError, "corrupt"):
                read_resume(*args)
            report["episodes"] *= 2
            path.write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, "prefix"):
                read_resume(*args)

    def test_initial_report_is_finite_json(self):
        report = acceptance([], [], 0.8, expected_cases=10)
        json.dumps(report, allow_nan=False)
        self.assertFalse(report["performance_pass"])

    def test_each_slot_must_pass_and_missing_baselines_cannot_pass(self):
        rows = [row(p) for p in ("pair", "agent-p1", "agent-p2")]
        self.assertFalse(acceptance(rows, [], 0.8)["performance_pass"])
        baselines = [row(p, False) for p in ("teacher", "random", "idle")]
        self.assertTrue(acceptance(rows, baselines, 0.8)["performance_pass"])
        rows[-1]["success"] = False
        self.assertFalse(acceptance(rows, baselines, 0.8)["performance_pass"])

    def test_tied_clears_require_a_measured_improvement(self):
        rows = [row(p) for p in ("pair", "agent-p1", "agent-p2")]
        equal = [row(p) for p in ("teacher", "random", "idle")]
        self.assertFalse(acceptance(rows, equal, 0.8)["performance_pass"])
        weaker = [row(p, deaths=1) for p in ("teacher", "random", "idle")]
        result = acceptance(rows, weaker, 0.8)
        self.assertTrue(result["performance_pass"])
        self.assertEqual(result["deployment_parity"], "pending")
        self.assertEqual(result["human_shared_play"], "not-assessed")

    def test_partial_report_cannot_pass(self):
        rows = [row(p) for p in ("pair", "agent-p1", "agent-p2")]
        baselines = [row(p, False) for p in ("teacher", "random", "idle")]
        result = acceptance(rows, baselines, 0.8, expected_cases=10)
        self.assertFalse(result["complete"])
        self.assertFalse(result["performance_pass"])
