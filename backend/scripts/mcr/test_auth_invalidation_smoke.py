import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import auth_invalidation_smoke as smoke


class RunnerSafety(unittest.TestCase):
    def test_refuses_relative_symlink_target_or_digest_mismatch(self):
        with self.assertRaises(smoke.SmokeError):
            smoke.frozen_binary(Path("relative"), "0")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            p = root / "frozen"
            p.write_bytes(b"fixture")
            p.chmod(0o700)
            digest = hashlib.sha256(p.read_bytes()).hexdigest()
            self.assertEqual(smoke.frozen_binary(p, digest), p.resolve())
            with self.assertRaises(smoke.SmokeError):
                smoke.frozen_binary(p, "0" * 64)
            link = root / "link"
            link.symlink_to(p)
            with self.assertRaises(smoke.SmokeError):
                smoke.frozen_binary(link, digest)
            for name in ["target", "mcr-build"]:
                (root / name).mkdir()
                q = root / name / "artifact"
                q.write_bytes(p.read_bytes())
                q.chmod(0o700)
                with self.assertRaises(smoke.SmokeError):
                    smoke.frozen_binary(q, digest)

    def test_zero_or_wrong_inventory_never_passes(self):
        for output in ["", "test result: ok. 0 passed; 0 failed;", "test result: ok. 1 passed; 0 failed;"]:
            with patch.object(smoke, "execute", return_value=type("Result", (), {"stdout": output, "returncode": 0})()):
                with self.assertRaises(smoke.SmokeError):
                    smoke.run_test(Path("/fixture"), {}, {}, "pure")


if __name__ == "__main__":
    unittest.main()
