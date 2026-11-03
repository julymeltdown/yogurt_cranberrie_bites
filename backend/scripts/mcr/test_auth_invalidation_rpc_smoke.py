import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import auth_invalidation_rpc_smoke as smoke


class RunnerSafety(unittest.TestCase):
    def test_exact_frozen_inventory_and_clean_source_required_before_cluster(self):
        with tempfile.TemporaryDirectory() as directory:
            p = Path(directory) / "binaries.json"
            p.write_text(json.dumps({"artifacts": {}}))
            with self.assertRaises(smoke.SmokeError):
                smoke.binaries(p)
            p.write_text(json.dumps({"artifacts": {k: {} for k in smoke.REQUIRED}, "source_commit": "other"}))
            with self.assertRaises(smoke.SmokeError):
                smoke.binaries(p)
            with self.assertRaises(smoke.SmokeError):
                smoke.binaries(Path("relative"))

    def test_zero_wrong_or_missing_test_inventory_never_passes(self):
        for output in ["", "test result: ok. 0 passed; 0 failed;", "test result: ok. 3 passed; 0 failed;"]:
            with patch.object(smoke, "execute", return_value=type("Result", (), {"stdout": output, "returncode": 0})()):
                with self.assertRaises(smoke.SmokeError):
                    smoke.test(Path("/fixture"), {}, {}, "pure_rpc", 4)


if __name__ == "__main__":
    unittest.main()
