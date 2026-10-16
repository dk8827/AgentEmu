"""Compact release records must preserve failures and bind to shipped weights."""

import copy
import json
from pathlib import Path
import unittest
from model_evidence import validate_bundle, validate_evidence

ROOT = Path(__file__).resolve().parents[1]


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        folder = ROOT / "models/doubledragon2-mission1-v2"
        self.manifest = json.loads((folder / "manifest.json").read_text())
        self.evidence = json.loads((folder / "evaluation.json").read_text())

    def test_all_bundles(self):
        for folder in (ROOT / "models").iterdir():
            if folder.is_dir():
                with self.subTest(model=folder.name):
                    validate_bundle(folder)

    def test_identity(self):
        for key in ("rom_sha256", "weights_sha256"):
            evidence = copy.deepcopy(self.evidence)
            evidence[key] = "0" * 64
            with self.assertRaises(ValueError):
                validate_evidence(evidence, self.manifest)

    def test_failure_cannot_be_changed_or_removed_without_changing_summary(self):
        for remove in (False, True):
            evidence = copy.deepcopy(self.evidence)
            report = next(
                r for r in evidence["reports"] if any(not c["success"] for c in r["cases"])
            )
            row = next(c for c in report["cases"] if not c["success"])
            if remove:
                report["cases"].remove(row)
            else:
                row["success"] = True
            with self.assertRaises(ValueError):
                validate_evidence(evidence, self.manifest)

    def test_duplicate_cases(self):
        self.evidence["reports"][0]["cases"].append(self.evidence["reports"][0]["cases"][0])
        with self.assertRaises(ValueError):
            validate_evidence(self.evidence, self.manifest)

    def test_bad_fields(self):
        for key, value in (
            ("frames", -1),
            ("success", 1),
            ("input_sha256", "bad"),
            ("deaths", [-1, 0]),
            ("survivors", [1, 1]),
        ):
            evidence = copy.deepcopy(self.evidence)
            evidence["reports"][0]["cases"][0][key] = value
            with self.assertRaises(ValueError):
                validate_evidence(evidence, self.manifest)

    def test_zero_clear_models_are_not_hidden(self):
        evidence = copy.deepcopy(self.evidence)
        for report in evidence["reports"]:
            for row in report["cases"]:
                row["success"] = False
            for score in report["summary"].values():
                score["clears"] = 0
        validate_evidence(evidence, self.manifest)


if __name__ == "__main__":
    unittest.main()
