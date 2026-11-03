import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import invalidation_clients_smoke as smoke


class RunnerSafety(unittest.TestCase):
    def test_frozen_inventory_and_clean_source_are_required_before_cluster(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "binaries.json"
            path.write_text(json.dumps({"artifacts": {}}))
            with self.assertRaises(smoke.SmokeError):
                smoke.binaries(path)
            path.write_text(json.dumps({"artifacts": {k: {} for k in smoke.REQUIRED}, "source_commit": "other"}))
            with self.assertRaises(smoke.SmokeError):
                smoke.binaries(path)
            with self.assertRaises(smoke.SmokeError):
                smoke.binaries(Path("relative"))

    def test_wrong_or_missing_test_inventory_never_passes(self):
        for output in ["", "test result: ok. 0 passed; 0 failed;", "test result: ok. 3 passed; 0 failed;"]:
            with patch.object(smoke, "execute", return_value=type("Result", (), {"stdout": output, "returncode": 0})()):
                with self.assertRaises(smoke.SmokeError):
                    smoke.test(Path("/fixture"), {}, {}, "validation", 4)

    def test_target_and_digest_mismatch_never_reach_postgres(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "target"
            target.mkdir()
            binary = target / "fixture"
            binary.write_bytes(b"fixture")
            binary.chmod(0o700)
            with self.assertRaises(smoke.SmokeError):
                smoke.frozen_binary(binary, smoke.digest(binary))
            copy = Path(directory) / "copy"
            copy.write_bytes(b"fixture")
            copy.chmod(0o700)
            with self.assertRaises(smoke.SmokeError):
                smoke.frozen_binary(copy, "0" * 64)


if __name__ == "__main__":
    unittest.main()
