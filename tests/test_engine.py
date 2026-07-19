from __future__ import annotations

import copy
import tempfile
import unittest
from pathlib import Path

from smart_hedge.config import DEFAULT_CONFIG, project_root
from smart_hedge.engine import SmartHedgeEngine


class EngineIntegrationTests(unittest.TestCase):
    def test_synthetic_recommendation_is_paper_only_and_replayable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config = copy.deepcopy(DEFAULT_CONFIG)
            config["_config_dir"] = str(project_root())
            config["provider"]["kind"] = "synthetic"
            config["provider"]["evidence_file"] = ""
            config["model"]["kind"] = "heuristic"
            config["storage"]["sqlite_path"] = str(Path(tmp) / "decisions.sqlite3")
            config["core"]["binary"] = str(project_root() / "build" / "smart_dynamic_hedge")
            config["core"]["auto_build"] = False
            engine = SmartHedgeEngine(config)
            value = engine.recommendation("SPY")
            self.assertEqual(value["mode"], "paper")
            self.assertFalse(value["policy"]["live_execution_allowed"])
            self.assertFalse(value["audit"]["broker_order_endpoint_present"])
            replay = engine.replay(value["decision_id"])
            self.assertTrue(replay["audit"]["stored_content_hash_valid"])
            self.assertEqual(replay["decision_id"], value["decision_id"])


if __name__ == "__main__":
    unittest.main()
