"""Real local Git histories test binding only; no application/runtime proof."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from in03_stack_runtime import InputRefusal
from in04_wall_runtime.launch import validate_frontend_binding


DOMAIN = "contracts/generated/mcr-domain.openapi.json"
REALTIME = "contracts/realtime/v1/world.schema.json"


class FrontendBindingCases(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="mcr-binding-git-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.git("init", "--quiet")
        for name, value in ((DOMAIN, {"fixture": "domain"}),
                            (REALTIME, {"fixture": "realtime"}),
                            ("frontend/package.json", {"version": "0.1.0"})):
            self.write(name, value)
        self.helper = self.commit("backend contract fixture")
        self.write("frontend/published-fixture.txt", "published frontend delta")
        self.frontend = self.commit("frontend fixture")
        self.publish()

    def git(self, *args):
        return subprocess.check_output(["git", "-c", "user.name=MCR binding test",
            "-c", "user.email=mcr-binding@example.test", *args], cwd=self.root,
            stderr=subprocess.PIPE).decode().strip()

    def write(self, name, value):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value) + "\n")

    def commit(self, message):
        self.git("add", "--all")
        self.git("commit", "--quiet", "-m", message)
        return self.git("rev-parse", "HEAD")

    def publish(self):
        self.git("update-ref", "refs/remotes/origin/master", self.git("rev-parse", "HEAD"))

    def test_distinct_published_frontend_keeps_actual_backend_contract_identity(self):
        self.assertNotEqual(self.helper, self.frontend)
        hashes = validate_frontend_binding(self.root, self.helper, self.frontend)
        self.assertEqual(set(hashes), {DOMAIN, REALTIME})
        # Later source changes never alter either immutable committed input.
        self.write(DOMAIN, {"uncommitted": "different"})
        self.assertEqual(validate_frontend_binding(self.root, self.helper, self.frontend), hashes)

    def test_unpublished_frontend_is_refused(self):
        self.write("frontend/published-fixture.txt", "private delta")
        private = self.commit("private frontend")
        with self.assertRaises(InputRefusal):
            validate_frontend_binding(self.root, self.helper, private)

    def test_unknown_commit_is_refused(self):
        with self.assertRaises(InputRefusal):
            validate_frontend_binding(self.root, self.helper, "f" * 40)

    def test_abbreviated_revision_is_refused(self):
        with self.assertRaises(InputRefusal):
            validate_frontend_binding(self.root, self.helper, self.frontend[:12])

    def test_changed_domain_cannot_be_reported_as_deployed(self):
        self.write(DOMAIN, {"fixture": "new domain"})
        candidate = self.commit("new domain")
        self.publish()
        with self.assertRaisesRegex(InputRefusal, "in04-frontend-contract-drift"):
            validate_frontend_binding(self.root, self.helper, candidate)

    def test_changed_realtime_cannot_be_reported_as_deployed(self):
        self.write(REALTIME, {"fixture": "new realtime"})
        candidate = self.commit("new realtime")
        self.publish()
        with self.assertRaisesRegex(InputRefusal, "in04-frontend-contract-drift"):
            validate_frontend_binding(self.root, self.helper, candidate)

    def test_published_revision_without_frontend_is_refused(self):
        (self.root / "frontend/package.json").unlink()
        candidate = self.commit("no app")
        self.publish()
        with self.assertRaises(InputRefusal):
            validate_frontend_binding(self.root, self.helper, candidate)


if __name__ == "__main__":
    unittest.main()
