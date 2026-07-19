from __future__ import annotations

import copy
import unittest
from datetime import datetime, timedelta, timezone

from smart_hedge.config import DEFAULT_CONFIG
from smart_hedge.models import Bar, FeatureSet, MarketSnapshot, ModelAssessment, Quote
from smart_hedge.policy import evaluate_policy


class PolicyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.config = copy.deepcopy(DEFAULT_CONFIG)
        self.config["policy"]["max_preview_notional"] = 1_000_000.0
        now = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        self.snapshot = MarketSnapshot(
            symbol="TEST",
            quote=Quote("TEST", 99.99, 100.01, 100.0, now, "unit-test", "open"),
            bars=[Bar(now, 100, 101, 99, 100, 1000)],
            evidence=[],
        )
        self.features = FeatureSet(
            values={"spread_bps": 2.0},
            missing=[],
            warnings=[],
            data_quality=1.0,
            evidence_ids=[],
        )
        self.core = {
            "inputs": {"current_shares": 0.0, "base_no_trade_band_shares": 2.0},
            "hedge": {"target_stock_shares": 10.0},
        }
        self.assessment = ModelAssessment(
            advisor_kind="test",
            model="test",
            regime="calm",
            confidence=0.9,
            hedge_urgency=0.3,
            band_multiplier=2.0,
            summary="test",
            evidence_ids=[],
            risks=[],
            scenario_spot_shocks=[-0.05, 0.05],
            data_requests=[],
        )

    def test_model_can_only_change_band_not_target(self) -> None:
        decision = evaluate_policy(
            self.config, self.snapshot, self.features, self.core, self.assessment
        )
        self.assertEqual(decision.target_stock_shares, 10.0)
        self.assertEqual(decision.effective_no_trade_band_shares, 4.0)
        self.assertEqual(decision.paper_trade_preview_shares, 10.0)
        self.assertFalse(decision.live_execution_allowed)

    def test_stale_quote_blocks_preview(self) -> None:
        stale = (datetime.now(timezone.utc) - timedelta(hours=1)).isoformat().replace("+00:00", "Z")
        self.snapshot.quote.timestamp = stale
        decision = evaluate_policy(
            self.config, self.snapshot, self.features, self.core, self.assessment
        )
        self.assertIn("STALE_QUOTE", decision.blocking_reasons)
        self.assertEqual(decision.paper_trade_preview_shares, 0.0)

    def test_unknown_model_citation_is_blocked(self) -> None:
        self.assessment.evidence_ids = ["invented-id"]
        decision = evaluate_policy(
            self.config, self.snapshot, self.features, self.core, self.assessment
        )
        self.assertIn("MODEL_CITED_UNKNOWN_EVIDENCE", decision.blocking_reasons)

    def test_low_confidence_cannot_change_band(self) -> None:
        self.assessment.confidence = 0.1
        decision = evaluate_policy(
            self.config, self.snapshot, self.features, self.core, self.assessment
        )
        self.assertEqual(decision.effective_no_trade_band_shares, 2.0)
        self.assertIn("model_confidence_too_low_for_band_change", decision.warnings)


if __name__ == "__main__":
    unittest.main()
