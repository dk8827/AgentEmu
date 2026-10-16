import unittest
from unittest.mock import patch
from contra_training.release_contract import check_death_metric


class MetricContractTests(unittest.TestCase):
    def test_new_metric_requires_the_versioned_ice_suite(self):
        with patch(
            "contra_training.release_contract._native.ICE_DEATH_METRIC",
            "cartridge-life-losses-v2",
            create=True,
        ):
            check_death_metric({"death_metric": "cartridge-life-losses-v2"}, "iceclimber-mountain1")
            with self.assertRaisesRegex(ValueError, "metric mismatch"):
                check_death_metric({}, "iceclimber-mountain1")
            check_death_metric({}, "snowbros-floor1")

    def test_old_binding_cannot_claim_new_life_accounting(self):
        with patch(
            "contra_training.release_contract._native.ICE_DEATH_METRIC",
            "death-transitions-v1",
            create=True,
        ):
            check_death_metric({}, "iceclimber-mountain1")
            with self.assertRaisesRegex(ValueError, "metric mismatch"):
                check_death_metric(
                    {"death_metric": "cartridge-life-losses-v2"}, "iceclimber-mountain1"
                )
