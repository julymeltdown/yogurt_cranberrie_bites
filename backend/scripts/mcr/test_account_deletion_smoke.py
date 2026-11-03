import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import account_deletion_smoke as smoke


class RunnerSafety(unittest.TestCase):
    def test_binary_refuses_relative_target_symlink_and_digest_mismatch(self):
        with self.assertRaises(smoke.SmokeError):
            smoke.binary(Path("relative"), "x")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            file = root / "fixture"
            file.write_text("synthetic")
            file.chmod(0o700)
            digest = hashlib.sha256(file.read_bytes()).hexdigest()
            self.assertEqual(smoke.binary(file, digest), file.resolve())
            with self.assertRaises(smoke.SmokeError):
                smoke.binary(file, "0" * 64)
            link = root / "linked"
            link.symlink_to(file)
            with self.assertRaises(smoke.SmokeError):
                smoke.binary(link, digest)
            target = root / "target"
            target.mkdir()
            artifact = target / "fixture"
            artifact.write_bytes(file.read_bytes())
            artifact.chmod(0o700)
            with self.assertRaises(smoke.SmokeError):
                smoke.binary(artifact, digest)

    def test_tests_cannot_report_success_without_exact_inventory(self):
        for stdout in ["", "test result: ok. 0 passed; 0 failed;", "test result: ok. 3 passed; 0 failed;"]:
            with patch.object(smoke, "execute", return_value=type("Result", (), {"stdout": stdout, "returncode": 0})()):
                with self.assertRaises(smoke.SmokeError):
                    smoke.run_tests(Path("/fixture"), {}, "pure", {})


if __name__ == "__main__":
    unittest.main()
