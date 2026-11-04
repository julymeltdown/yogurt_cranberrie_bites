"""Deterministic tiny Git fixtures; no app, browser, Cargo, services or credentials."""
import copy
import contextlib
import gzip
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from check import COMMAND_SCHEMA, FRONTEND_GATES, SCHEMA, Rejected, digest, evaluate, vitest_counts

TAP = b"""TAP version 13
# Subtest: deterministic assertion
ok 1 - deterministic assertion
1..1
# tests 1
# suites 0
# pass 1
# fail 0
# cancelled 0
# skipped 0
# todo 0
"""
EVIDENCE = "docs/mcr/evidence/release/fixture"
SOURCE = "scripts/mcr/release-check.sh"
TEST_SOURCE = "frontend/src/example.test.ts"
NODE_SOURCE = "frontend/e2e/unit.node.mjs"


class ReleaseChecks(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="mcr-qa10-fixture-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.git("init", "-q")
        self.git("config", "user.name", "Deterministic Fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.write(SOURCE, b"#!/bin/sh\nexit 0\n")
        self.write(TEST_SOURCE, b"import {test} from 'vitest'; // Synthetic source; not executed.\n")
        self.write(NODE_SOURCE, b"import {test} from 'node:test'; // Synthetic source; not executed.\n")
        self.write("contracts/fixture.json", b"{}\n")
        self.write("generated/TASKS_EN.json", json.dumps({"tasks": [
            {"id": "QA10", "priority": "P0", "owned_paths": [SOURCE]},
            {"id": "QA08", "priority": "P0", "owned_paths": ["frontend/e2e/qa08/"]},
            {"id": "FE01", "priority": "P0", "owned_paths": ["FS/example.test.ts"]},
        ]}).encode())
        self.git("add", ".")
        self.commit("fixture initial source")
        self.sha = self.git("rev-parse", "HEAD")
        self.candidate = {"commit": self.sha, "tree": self.git("rev-parse", "HEAD^{tree}"), "dirty": False}
        self.pins = {p: {"git_blob": self.git("rev-parse", self.sha + ":" + p),
                         "sha256": digest((self.root / p).read_bytes())} for p in [SOURCE, TEST_SOURCE, NODE_SOURCE]}
        self.test = {"schema": COMMAND_SCHEMA, "id": "unit", "kind": "tests", "status": "PASS",
                     "candidate": self.candidate, "same_candidate_end": True, "source_pins": self.pins,
                     "test_files": [TEST_SOURCE],
                     "command": {"argv": ["node", "--test", TEST_SOURCE], "cwd": ".", "attempted": True,
                                 "exit_code": 0, "signal": None},
                     "log": self.ref("unit.log", TAP),
                     "report": {"format": "tap", "artifact": self.ref("unit.tap", TAP)}}
        self.production = {"build_receipt": "build", "root": EVIDENCE + "/site",
                           "modules": ["frontend/src/main.ts"],
                           "assets": [self.ref("site/app.js", b"console.log('public application');\n")]}
        command = {"argv": ["vite", "build"], "cwd": "frontend", "attempted": True,
                   "exit_code": 0, "signal": None}
        self.build = {"schema": COMMAND_SCHEMA, "id": "build", "kind": "build", "status": "PASS",
                      "candidate": self.candidate, "same_candidate_end": True, "source_pins": self.pins,
                      "command": command, "log": self.ref("build.log", b"fixture build report\n"),
                      "report": {"format": "execution-json", "artifact": self.ref("build.json", self.jsonbytes({
                          "candidate": self.candidate, "command": command, "status": "PASS",
                          "production": {k: v for k, v in self.production.items() if k != "build_receipt"}}))}}
        self.manifest = {"schema": SCHEMA, "candidate": self.candidate,
                         "release_status": "INCOMPLETE", "receipts": [], "production": self.production,
                         "task_links": [{"task_id": "QA10", "commit": self.sha,
                                         "paths": [SOURCE], "receipt_ids": ["unit"]}]}

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, stderr=subprocess.DEVNULL).decode().strip()

    def commit(self, message):
        subprocess.run(["git", "commit", "-qm", message], cwd=self.root, check=True,
                       env={**os.environ, "GIT_AUTHOR_DATE": "2026-01-01T00:00:00Z",
                            "GIT_COMMITTER_DATE": "2026-01-01T00:00:00Z"})

    @staticmethod
    def jsonbytes(value):
        return (json.dumps(value, sort_keys=True) + "\n").encode()

    def write(self, path, value):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(value)

    def ref(self, path, data):
        name = EVIDENCE + "/" + path
        self.write(name, data)
        return {"path": name, "sha256": digest(data)}

    def run_check(self):
        self.manifest["receipts"] = [self.ref("unit.json", self.jsonbytes(self.test)),
                                     self.ref("build-receipt.json", self.jsonbytes(self.build))]
        self.write(EVIDENCE + "/manifest.json", self.jsonbytes(self.manifest))
        return evaluate(self.root, self.sha, EVIDENCE + "/manifest.json")

    def assert_rule(self, rule):
        report = self.run_check()
        self.assertEqual(report["status"], "FAIL", report)
        self.assertIn(rule, report["findings"])

    def test_positive_exact_candidate_test_artifacts_production_and_task_link(self):
        report = self.run_check()
        self.assertEqual(report["source_checks"], "PASS", report)
        self.assertEqual(report["status"], "PARTIAL")
        self.assertEqual(report["release_status"], "INCOMPLETE")
        self.assertEqual(report["task_links"][0]["acceptance"], "NOT_ASSESSED")
        self.assertEqual(report["missing_required_tasks"], ["FE01", "QA08"])
        self.assertTrue(all(g["status"] == "NOT_RUN" for g in report["live_gates"]))

    def test_absent_manifest_is_incomplete_not_pass(self):
        report = evaluate(self.root, self.sha)
        self.assertEqual(report["status"], "INCOMPLETE")
        self.assertIn("manifest.NOT_RUN", report["findings"])

    def test_mismatched_receipt_candidate_is_rejected(self):
        self.test["candidate"] = {**self.candidate, "commit": "f" * 40}
        self.assert_rule("candidate.mismatch")

    def test_wrong_tree_dirty_and_missing_end_fence_are_rejected(self):
        original = copy.deepcopy(self.test)
        for field, value, rule in [("candidate", {**self.candidate, "tree": "f" * 40}, "candidate.tree-mismatch"),
                                   ("candidate", {**self.candidate, "dirty": True}, "candidate.dirty"),
                                   ("same_candidate_end", False, "candidate.end-fence-missing")]:
            with self.subTest(field=field, value=value):
                self.test = copy.deepcopy(original)
                self.test[field] = value
                self.assert_rule(rule)

    def test_old_ancestor_cannot_stand_in_for_current_candidate(self):
        self.write("new-source.txt", b"new revision\n")
        self.git("add", "new-source.txt")
        self.commit("new candidate")
        result = evaluate(self.root, self.sha)
        self.assertEqual(result["findings"], ["candidate.not-current-head"])

    def test_failed_unattempted_signalled_or_boolean_exit_cannot_pass(self):
        for change in [{"exit_code": 1}, {"attempted": False}, {"signal": "SIGTERM"}, {"exit_code": False}]:
            with self.subTest(change=change):
                self.test["command"].update({"attempted": True, "exit_code": 0, "signal": None})
                self.test["command"].update(change)
                self.assert_rule("command.false-pass")

    def test_hash_tampering_is_rejected(self):
        self.write(self.test["report"]["artifact"]["path"], b"changed report\n")
        self.assert_rule("artifact.hash-mismatch")

    def test_fabricated_tap_pass_with_no_test_points_is_rejected(self):
        data = TAP.replace(b"ok 1 - deterministic assertion\n", b"")
        self.test["report"]["artifact"] = self.ref("unit.tap", data)
        self.assert_rule("tests.tap-plan")

    def test_fabricated_pass_count_is_rejected(self):
        self.test["report"]["artifact"] = self.ref("unit.tap", TAP.replace(b"# tests 1", b"# tests 99"))
        self.assert_rule("tests.tap-empty-or-fabricated-count")

    def test_skipped_test_remains_an_incomplete_gap(self):
        data = TAP.replace(b"1..1\n", b"ok 2 - skipped assertion # SKIP fixture not available\n1..2\n")
        data = data.replace(b"# tests 1", b"# tests 2").replace(b"# skipped 0", b"# skipped 1")
        self.test["report"]["artifact"] = self.ref("unit.tap", data)
        report = self.run_check()
        self.assertEqual(report["status"], "INCOMPLETE", report)
        self.assertIn("tests.skipped-cases", report["findings"])

    def test_source_pin_and_dispatched_inventory_are_verified(self):
        original = self.test["source_pins"][SOURCE]["sha256"]
        self.test["source_pins"][SOURCE]["sha256"] = "f" * 64
        self.assert_rule("source.hash-mismatch")
        self.test["source_pins"][SOURCE]["sha256"] = original
        self.test["command"]["argv"].remove(TEST_SOURCE)
        self.assert_rule("tests.not-dispatched")

    def test_mock_module_rejected_even_with_rehashed_build_report(self):
        for path in ["frontend/src/world/network/mock.ts", "frontend/src/world/network/MockWorldTransport.ts"]:
            with self.subTest(path=path):
                self.production["modules"] = ["frontend/src/main.ts", path]
                self.repin_production()
                self.assert_rule("production.mock-module")

    def repin_production(self):
        report = {"candidate": self.candidate, "command": self.build["command"], "status": "PASS",
                  "production": {k: v for k, v in self.production.items() if k != "build_receipt"}}
        self.build["report"]["artifact"] = self.ref("build.json", self.jsonbytes(report))

    def test_synthetic_secret_marker_is_rejected_without_value_disclosure(self):
        for marker, rule in [(b"-----BEGIN PRIVATE KEY-----", "production.private-key"),
                             (b"client_secret='synthetic-not-a-real-secret'", "production.credential-literal"),
                             (b'{"client_secret":"synthetic-not-a-real-secret"}', "production.credential-literal"),
                             (b"eyJ1234567890.abcdefghijk.abcdefghijk", "production.jwt-literal"),
                             (b"new MockWorldTransport()", "production.mock-marker")]:
            with self.subTest(rule=rule):
                self.production["assets"] = [self.ref("site/app.js", marker)]
                self.repin_production()
                result = self.run_check()
                self.assertIn(rule, result["findings"])
                self.assertNotIn(marker.decode(), json.dumps(result))

    def test_extra_or_unbound_production_assets_are_rejected(self):
        self.write(EVIDENCE + "/site/omitted.js", b"unlisted\n")
        self.assert_rule("production.omitted-assets")
        (self.root / EVIDENCE / "site/omitted.js").unlink()
        self.production["modules"].append("frontend/src/changed.ts")
        self.assert_rule("production.build-report-mismatch")

    def test_secret_file_is_refused_without_reading_its_content(self):
        name = EVIDENCE + "/site/.env.production"
        self.write(name, b"SYNTHETIC_PRIVATE_VALUE=do-not-copy\n")
        self.production["assets"].append({"path": name, "sha256": "0" * 64})
        self.repin_production()
        self.assert_rule("path.secret-input-refused")

    def test_unsafe_path_and_symlink_cannot_read_outside_evidence(self):
        original = self.test["log"]
        for path in ["../outside.log", "/outside.log", EVIDENCE + "/../outside.log"]:
            self.test["log"] = {"path": path, "sha256": "0" * 64}
            self.assert_rule("path.unsafe")
        self.test["log"] = original
        target = self.root / original["path"]
        target.unlink()
        target.symlink_to(self.root / SOURCE)
        self.assert_rule("path.symlink-refused")

    def test_task_id_path_commit_and_evidence_link_are_not_just_labels(self):
        link = self.manifest["task_links"][0]
        original = copy.deepcopy(link)
        for changes, rule in [({"task_id": "INVENTED"}, "trace.unknown-or-duplicate-task"),
                              ({"receipt_ids": ["missing"]}, "trace.missing-source-or-evidence"),
                              ({"paths": [TEST_SOURCE]}, "trace.path-outside-task")]:
            self.manifest["task_links"][0] = {**original, **changes}
            self.assert_rule(rule)

    def test_stale_task_source_commit_is_rejected_even_when_ancestor(self):
        old = self.sha
        self.write(SOURCE, b"#!/bin/sh\nexit 1\n")
        self.git("add", SOURCE)
        self.commit("changed implementation")
        self.sha = self.git("rev-parse", "HEAD")
        self.candidate.update(commit=self.sha, tree=self.git("rev-parse", "HEAD^{tree}"))
        self.pins[SOURCE] = {"git_blob": self.git("rev-parse", self.sha + ":" + SOURCE),
                             "sha256": digest((self.root / SOURCE).read_bytes())}
        self.manifest["task_links"][0]["commit"] = old
        self.repin_production()
        self.assert_rule("trace.stale-source-commit")

    def test_legitimate_not_run_receipt_stays_incomplete(self):
        self.test.update(status="NOT_RUN", reason="No device assigned", owner="device owner")
        self.manifest["task_links"] = []
        result = self.run_check()
        self.assertEqual(result["status"], "INCOMPLETE", result)
        self.assertEqual(result["receipts"][0]["status"], "NOT_RUN")

    def test_duplicate_receipts_and_false_global_pass_are_rejected(self):
        self.build["id"] = "unit"
        self.assert_rule("receipt.duplicate-or-invalid-id")
        self.build["id"] = "build"
        self.manifest["release_status"] = "PASS"
        self.assert_rule("manifest.false-release-pass")

    def test_missing_production_remains_incomplete(self):
        self.manifest.pop("production")
        report = self.run_check()
        self.assertEqual(report["source_checks"], "INCOMPLETE")
        self.assertIn("production.NOT_RUN", report["findings"])

    def test_duplicate_json_keys_are_rejected(self):
        self.write(EVIDENCE + "/duplicate.json", b'{"schema":1,"schema":2}')
        report = evaluate(self.root, self.sha, EVIDENCE + "/duplicate.json")
        self.assertEqual(report["findings"], ["json.duplicate-key"])

    def test_vitest_report_counts_files_and_failures_are_checked(self):
        report = {"success": True, "numTotalTests": 1, "numPassedTests": 1, "numFailedTests": 0,
                  "numPendingTests": 0, "testResults": [{"name": "/private/fixture/" + TEST_SOURCE,
                                                       "assertionResults": [{"status": "passed"}]}]}
        self.assertEqual(vitest_counts(self.jsonbytes(report), [TEST_SOURCE])["total"], 1)
        report["numTotalTests"] = 99
        with self.assertRaisesRegex(Rejected, "tests.vitest-fabricated-count"):
            vitest_counts(self.jsonbytes(report), [TEST_SOURCE])
        report["numTotalTests"] = 1
        report["testResults"][0]["assertionResults"][0]["status"] = "failed"
        with self.assertRaisesRegex(Rejected, "tests.vitest-fabricated-count"):
            vitest_counts(self.jsonbytes(report), [TEST_SOURCE])

    def frontend_fixture(self):
        inventory = {"vitest": [TEST_SOURCE], "node": [NODE_SOURCE], "playwright": [], "unsupported": []}
        vitest = {"success": True, "numTotalTests": 1, "numPassedTests": 1, "numFailedTests": 0,
                  "numPendingTests": 0, "testResults": [{"name": TEST_SOURCE,
                                                       "assertionResults": [{"status": "passed"}]}]}
        files = {"vitest.json": self.jsonbytes(vitest), "node-tests.log": TAP,
                 "test-inventory.json": self.jsonbytes(inventory),
                 "build-audit.json": self.jsonbytes({"modules": ["frontend/src/main.ts"],
                                                     "assets": [{"path": "app.js", "bytes": 1, "sha256": "f" * 64}],
                                                     "findings": []})}
        commands = []
        for gate in sorted(FRONTEND_GATES):
            log = "node-tests.log" if gate == "node-tests" else gate + ".log"
            files.setdefault(log, b"synthetic command fixture\n")
            commands.append({"id": gate, "status": "PASS", "attempted": True, "exit_code": 0,
                             "signal": None, "nonempty_count": 1, "cwd": ".", "log": log,
                             "argv": ["vitest", TEST_SOURCE] if gate == "vitest" else
                             ["node", "--test", NODE_SOURCE] if gate == "node-tests" else ["synthetic", gate],
                             "report_hash": digest(files[log])})
        for path, data in files.items():
            self.ref(path, data)
        report = {"schema": "mcr.qa10.frontend.v1", "candidate": self.candidate, "status": "PARTIAL",
                  "same_candidate_end": True, "unsupported": [{"status": "NOT_RUN", "criterion": "live services"}],
                  "source_pins": self.pins, "contract_pins": {"contracts/fixture.json": {
                      "git_blob": self.git("rev-parse", self.sha + ":contracts/fixture.json"),
                      "sha256": digest(b"{}\n")}}, "commands": commands,
                  "test_inventory": inventory, "artifact_pins": {p: digest(d) for p, d in files.items()}}
        return report, files

    def test_existing_frontend_schema_is_consumed_and_archived_json_checked(self):
        report, files = self.frontend_fixture()
        self.write(EVIDENCE + "/vitest.json.gz", gzip.compress(files["vitest.json"], mtime=0))
        (self.root / EVIDENCE / "vitest.json").unlink()
        self.manifest.update(receipts=[self.ref("frontend.json", self.jsonbytes(report))],
                             task_links=[{"task_id": "QA10", "commit": self.sha,
                                          "paths": [SOURCE], "receipt_ids": ["frontend"]}], production=None)
        self.write(EVIDENCE + "/manifest.json", self.jsonbytes(self.manifest))
        result = evaluate(self.root, self.sha, EVIDENCE + "/manifest.json")
        self.assertEqual(result["status"], "INCOMPLETE", result)
        self.assertEqual(result["receipts"][0]["counts"]["total"], 2)
        self.assertEqual(result["receipts"][0]["counts"]["production_bytes"], "NOT_RUN")

    def test_frontend_archives_remain_bounded_after_format_allowance(self):
        report, files = self.frontend_fixture()
        self.write(EVIDENCE + "/vitest.json.gz", gzip.compress(files["vitest.json"], mtime=0))
        (self.root / EVIDENCE / "vitest.json").unlink()
        self.manifest.update(receipts=[self.ref("frontend.json", self.jsonbytes(report))], task_links=[], production=None)
        self.write(EVIDENCE + "/manifest.json", self.jsonbytes(self.manifest))
        with patch("check.VITEST_LIMIT", 16):
            result = evaluate(self.root, self.sha, EVIDENCE + "/manifest.json")
        self.assertEqual(result["status"], "FAIL")
        self.assertIn("frontend.archive-too-large", result["findings"])

    def test_frontend_claim_cannot_hide_failed_command_or_renamed_tests(self):
        report, _ = self.frontend_fixture()
        report["commands"][0]["exit_code"] = 1
        self.manifest.update(receipts=[self.ref("frontend.json", self.jsonbytes(report))], task_links=[], production=None)
        self.write(EVIDENCE + "/manifest.json", self.jsonbytes(self.manifest))
        result = evaluate(self.root, self.sha, EVIDENCE + "/manifest.json")
        self.assertEqual(result["status"], "FAIL")
        self.assertIn("command.false-pass", result["findings"])

    def test_frontend_inventory_cannot_drop_a_candidate_test_file(self):
        report, _ = self.frontend_fixture()
        report["test_inventory"]["vitest"] = [SOURCE]
        value = self.jsonbytes(report["test_inventory"])
        self.ref("test-inventory.json", value)
        report["artifact_pins"]["test-inventory.json"] = digest(value)
        self.manifest.update(receipts=[self.ref("frontend.json", self.jsonbytes(report))], task_links=[], production=None)
        self.write(EVIDENCE + "/manifest.json", self.jsonbytes(self.manifest))
        result = evaluate(self.root, self.sha, EVIDENCE + "/manifest.json")
        self.assertIn("frontend.candidate-test-inventory", result["findings"])

    def test_cli_exit_codes_distinguish_partial_incomplete_and_fail(self):
        self.run_check()
        script = Path(__file__).with_name("check.py")
        command = [sys.executable, "-B", str(script), "--root", str(self.root), "--candidate", self.sha]
        for arguments, code, status in [(["--manifest", EVIDENCE + "/manifest.json"], 0, "PARTIAL"),
                                         ([], 2, "INCOMPLETE"), (["--candidate", "f" * 40], 1, "FAIL")]:
            with self.subTest(code=code):
                # Exercise the actual entrypoint without a nested Python child;
                # the source-round supervisor runs external CLI probes separately.
                from check import main
                output = io.StringIO()
                with patch.object(sys, "argv", command[2:] + arguments), contextlib.redirect_stdout(output):
                    result = main()
                self.assertEqual(result, code)
                self.assertEqual(json.loads(output.getvalue())["status"], status)

    def test_boolean_vitest_count_is_rejected(self):
        report = {"success": True, "numTotalTests": True, "numPassedTests": 1, "numFailedTests": 0,
                  "testResults": [{"name": TEST_SOURCE, "assertionResults": [{"status": "passed"}]}]}
        with self.assertRaisesRegex(Rejected, "tests.vitest-count-shape"):
            vitest_counts(self.jsonbytes(report), [TEST_SOURCE])

    def test_shell_command_cannot_masquerade_as_node_tests(self):
        self.test["command"]["argv"][0] = "sh"
        self.assert_rule("tests.runner-mismatch")

    def test_dirty_candidate_is_rejected(self):
        self.write(SOURCE, b"changed tracked source\n")
        result = self.run_check()
        self.assertEqual(result["findings"], ["candidate.worktree-dirty"])


if __name__ == "__main__":
    unittest.main()
