from __future__ import annotations

import unittest

from smart_hedge.model_advisor import validate_assessment_payload


class ModelSchemaTests(unittest.TestCase):
    def valid(self):
        return {
            "regime": "uncertain",
            "confidence": 0.5,
            "hedge_urgency": 0.5,
            "band_multiplier": 1.0,
            "summary": "No strong regime.",
            "evidence_ids": [],
            "risks": [],
            "scenario_spot_shocks": [-0.05, 0.05],
            "data_requests": [],
        }

    def test_valid_payload(self) -> None:
        result = validate_assessment_payload(self.valid(), "test", "test")
        self.assertEqual(result.regime, "uncertain")

    def test_extra_trade_field_rejected(self) -> None:
        payload = self.valid()
        payload["buy_shares"] = 100
        with self.assertRaises(ValueError):
            validate_assessment_payload(payload, "test", "test")

    def test_out_of_range_band_rejected(self) -> None:
        payload = self.valid()
        payload["band_multiplier"] = 50
        with self.assertRaises(ValueError):
            validate_assessment_payload(payload, "test", "test")


if __name__ == "__main__":
    unittest.main()
