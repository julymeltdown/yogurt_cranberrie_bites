"""Synthetic parser fixtures only. No MCR acceptance, app, Cargo or devices run."""
import copy
import contextlib
import hashlib
import json
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check as source
from composition.model import AUTHORITY, GATES, SCHEMA, evaluate
from composition.reports import SCHEMA as EVIDENCE_SCHEMA, playwright_counts, rust_counts, unittest_counts

ROOT = Path(__file__).resolve().parents[4]
FOLDER = "docs/mcr/evidence/release/parser-fixture"
TAP = b"TAP version 13\nok 1 - assertion\n1..1\n# tests 1\n# suites 0\n# pass 1\n# fail 0\n# cancelled 0\n# skipped 0\n# todo 0\n"
RUST = b"\nrunning 1 test\ntest actual_native_scenario ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"


class Composition(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mcr-qa10-composition-fixture-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "user.name", "Parser Fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.plan = json.loads((ROOT / "generated/TASKS_EN.json").read_bytes())
        self.ia = json.loads((ROOT / "generated/IA_EN.json").read_bytes())
        self.paths = {}
        for row in self.plan["tasks"]:
            if row["priority"] != "P0":
                continue
            p = row["owned_paths"][0]
            first, sep, tail = p.partition("/")
            p = source.ALIASES.get(first, first) + (sep + tail if sep else "")
            if p.endswith("/"):
                p += "parser-fixture.txt"
            self.paths[row["id"]] = p
            self.write(p, b"Synthetic parser source, never executed.\n")
        self.node = "frontend/e2e/qa10-parser.node.mjs"
        self.vitest = "frontend/src/qa10-parser.test.ts"
        self.native = "backend/services/concert-service/tests/qa10_parser.rs"
        self.write(self.node, b"import {test} from 'node:test'; // parser fixture only\n")
        self.write(self.vitest, b"import {test} from 'vitest'; // parser fixture only\n")
        self.write(self.native, b"// parser fixture only\n")
        self.write("contracts/parser-fixture.json", b"{}\n")
        self.write(AUTHORITY, b"| QA08 | DEFERRED_BY_USER |\nDefer QA08; preserve 111 active tasks.\n")
        self.write("generated/TASKS_EN.json", self.bytes(self.plan))
        self.write("generated/IA_EN.json", self.bytes(self.ia))
        self.git("add", ".")
        self.commit("Synthetic parser sources, not application evidence")
        self.sha = self.git("rev-parse", "HEAD")
        self.candidate = {"commit": self.sha, "tree": self.git("rev-parse", "HEAD^{tree}"), "dirty": False}
        self.pins = {p: self.pin(p) for p in set(self.paths.values()) | {self.node, self.native, self.vitest}}
        self.command = {"argv": ["node", "--test", self.node], "cwd": ".", "attempted": True,
                        "exit_code": 0, "signal": None}
        self.source_test = {"schema": source.COMMAND_SCHEMA, "id": "source-tests", "kind": "tests", "status": "PASS",
                            "candidate": self.candidate, "same_candidate_end": True, "command": self.command,
                            "source_pins": self.pins, "test_files": [self.node], "log": self.ref("source.tap", TAP),
                            "report": {"format": "tap", "artifact": self.ref("source-report.tap", TAP)}}
        production = {"build_receipt": "build", "root": FOLDER + "/site", "modules": ["frontend/src/main.ts"],
                      "assets": [self.ref("site/app.js", b"console.log('public parser fixture');\n")]}
        build_command = {**self.command, "argv": ["vite", "build"]}
        self.build = {"schema": source.COMMAND_SCHEMA, "id": "build", "kind": "build", "status": "PASS",
                      "candidate": self.candidate, "same_candidate_end": True, "command": build_command,
                      "source_pins": self.pins, "log": self.ref("build.log", b"synthetic build record\n"),
                      "report": {"format": "execution-json", "artifact": self.ref("build-report.json", self.bytes({
                          "candidate": self.candidate, "command": build_command, "status": "PASS",
                          "production": {k: v for k, v in production.items() if k != "build_receipt"}}))}}
        self.source_manifest = {"schema": source.SCHEMA, "candidate": self.candidate, "release_status": "INCOMPLETE",
                                "receipts": [self.ref("source-test.json", self.bytes(self.source_test)), self.ref("build.json", self.bytes(self.build))],
                                "production": production, "task_links": [{"task_id": "QA10", "commit": self.sha,
                                    "paths": [self.paths["QA10"]], "receipt_ids": ["source-tests"]}]}
        self.reports = {}
        for identity, category, level in [("backend", "backend", "native"), ("frontend", "frontend", "unit"),
                                          ("live", "service", "live"), ("ios", "device", "device"),
                                          ("android", "device", "device"), ("operator", "operator", "operator")]:
            self.reports[identity] = self.make_report(identity, category, level)
        self.reports["frontend"] = self.frontend_report()
        self.manifest = {"schema": SCHEMA, "candidate": self.candidate, "source_manifest": None,
                         "receipts": [], "tasks": [], "ia": [], "gates": [], "deferrals": [{
                             "task_id": "QA08", "status": "DEFERRED_BY_USER", "blocking": False,
                             "authority": {"path": AUTHORITY, **self.pin(AUTHORITY)}}]}
        for identity, path in self.paths.items():
            self.manifest["tasks"].append(self.record(identity, [path], ["backend"],
                                                      "DEFERRED_BY_USER" if identity == "QA08" else "ACCEPTED"))
        for row in self.ia["screens"]:
            if row["priority"] == "P0":
                self.manifest["ia"].append(self.record(row["id"], [self.paths[row["task_ids"][0]]], ["live"]))
        for gate, levels in GATES.items():
            receipts = ["ios", "android"] if gate == "QA06" else ["backend"] if gate == "QA01" else ["operator"] if gate == "QA02" else ["backend"] if "native" in levels else ["live"]
            self.manifest["gates"].append(self.record(gate, [self.native], receipts))
        self.manifest["gates"].append(self.record("QA08", [], [], "DEFERRED_BY_USER"))

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, stderr=subprocess.DEVNULL).decode().strip()

    def commit(self, message):
        subprocess.run(["git", "commit", "-qm", message], cwd=self.root, check=True, env={**os.environ,
                       "GIT_AUTHOR_DATE": "2026-01-01T00:00:00Z", "GIT_COMMITTER_DATE": "2026-01-01T00:00:00Z"})

    @staticmethod
    def bytes(value):
        return (json.dumps(value, sort_keys=True) + "\n").encode()

    def write(self, path, data):
        p = self.root / path
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data)

    def pin(self, path):
        return {"git_blob": self.git("rev-parse", self.sha + ":" + path),
                "sha256": hashlib.sha256((self.root / path).read_bytes()).hexdigest()}

    def ref(self, name, data):
        path = FOLDER + "/" + name
        self.write(path, data)
        return {"path": path, "sha256": hashlib.sha256(data).hexdigest()}

    def record(self, identity, paths, receipts, status="ACCEPTED"):
        return {"id": identity, "status": status, "owner": "parser-fixture-owner", "reason": "synthetic consistency fixture",
                "commit": self.sha, "paths": paths, "receipt_ids": receipts}

    def make_report(self, identity, category, level):
        command = copy.deepcopy(self.command)
        fmt, data, files = "tap", TAP, [self.node]
        if level == "native":
            command["argv"] = ["cargo", "test", "--test", "qa10_parser", "--", "--ignored"]
            fmt, data, files = "rust-libtest", RUST, [self.native]
        elif level != "unit":
            command["argv"] = ["node", "--test", self.node]
            fmt = "observations-json"
            data = self.bytes({"schema": "mcr.qa10.observations.v1", "candidate": self.candidate, "command": command,
                               "cases": [{"id": "observed-boundary", "status": "PASS",
                                          "artifacts": [self.ref(identity + "-observation.txt", b"synthetic observation fixture\n")]}]})
        observation = self.ref(identity + "-context.txt", b"synthetic context fixture; not real services\n")
        context = {"candidate": copy.deepcopy(self.candidate), "mock_mode": False, "synthetic": False,
                   "services": [{"id": "concert", "candidate": copy.deepcopy(self.candidate), "binary_sha256": "ab" * 32,
                                 "observation": observation}], "actors": ["11" * 32, "22" * 32],
                   "database": None, "device": None, "operator": None}
        if level == "native":
            context["database"] = {"engine": "postgresql", "version": "16.14", "isolated": True, "observation": observation}
        if level == "device":
            context["device"] = {"platform": identity, "physical": True, "hardware": "fixture", "os_version": "fixture",
                                 "browser": "Safari" if identity == "ios" else "Chrome", "browser_version": "fixture",
                                 "widths": [360, 390, 430], "themes": ["REVENGE", "PARADE", "DANGER"], "observation": observation}
        if level == "operator":
            context["operator"] = {k: observation for k in ("deployment", "retention", "place_provenance", "incident_controls")}
        def bound(name, facts):
            return self.ref(identity + "-" + name + "-bound.json", self.bytes({
                "schema": "mcr.qa10.context-observation.v1", "candidate": self.candidate,
                "facts": facts, "artifacts": [observation]}))
        for service in context["services"]:
            service["observation"] = bound("service", {"kind": "service", "id": service["id"], "binary_sha256": service["binary_sha256"]})
        context["actors"] = [{"fingerprint": fingerprint, "observation": bound("actor-" + str(index),
                              {"kind": "actor", "fingerprint": fingerprint})}
                             for index, fingerprint in enumerate(context["actors"])]
        for kind in ("database", "device"):
            if context[kind] is not None:
                context[kind]["observation"] = bound(kind, {"kind": kind, **{k: v for k, v in context[kind].items() if k != "observation"}})
        if level == "operator":
            context["operator"] = {k: bound(k, {"kind": "operator", "setting": k, "status": "VERIFIED"}) for k in context["operator"]}
        return {"schema": EVIDENCE_SCHEMA, "id": identity, "candidate": copy.deepcopy(self.candidate), "same_candidate_end": True,
                "status": "PASS", "owner": "parser-fixture-owner", "category": category, "level": level,
                "command": command, "source_pins": self.pins, "test_files": files, "log": self.ref(identity + ".log", data),
                "report": {"format": fmt, "artifact": self.ref(identity + "-report", data)}, "context": context,
                "coverage": {"task_ids": sorted(set(self.paths) - {"QA08"}),
                             "ia_ids": [r["id"] for r in self.ia["screens"] if r["priority"] == "P0"],
                             "gate_ids": sorted(GATES)}}

    def frontend_report(self):
        inventory = {"node": [self.node], "vitest": [self.vitest], "playwright": [], "unsupported": []}
        values = {"test-inventory.json": self.bytes(inventory), "node-tests.log": TAP,
                  "vitest.json": self.bytes({"numTotalTests": 1, "numPassedTests": 1, "numFailedTests": 0,
                      "numPendingTests": 0, "success": True, "testResults": [{"name": self.vitest,
                      "status": "passed", "assertionResults": [{"status": "passed"}]}]}),
                  "build-audit.json": self.bytes({"assets": [{"path": "app.js", "sha256": "ab" * 32}],
                                                  "modules": ["frontend/src/main.ts"], "findings": []})}
        commands = []
        for gate in sorted(source.FRONTEND_GATES):
            log = gate + ".log"
            values.setdefault(log, b"synthetic frontend command record\n")
            argv = ["node", "--test", self.node] if gate == "node-tests" else ["node", "vitest", self.vitest] if gate == "vitest" else ["node", "fixture-command"]
            commands.append({**self.command, "id": gate, "argv": argv, "log": log, "status": "PASS",
                             "nonempty_count": 1, "report_hash": source.digest(values[log])})
        for path, data in values.items():
            self.ref("frontend/" + path, data)
        return {"schema": "mcr.qa10.frontend.v1", "candidate": self.candidate, "same_candidate_end": True,
                "status": "PARTIAL", "unsupported": [{"criterion": "actual product", "status": "NOT_RUN"}],
                "source_pins": self.pins, "contract_pins": {"contracts/parser-fixture.json": self.pin("contracts/parser-fixture.json")},
                "artifact_pins": {p: source.digest(b) for p, b in values.items()}, "test_inventory": inventory, "commands": commands}

    def run_check(self):
        self.manifest["source_manifest"] = self.ref("source-manifest.json", self.bytes(self.source_manifest))
        self.manifest["receipts"] = [self.ref(("frontend/" if v["schema"] == "mcr.qa10.frontend.v1" else "")
                                             + k + ".json", self.bytes(v)) for k, v in self.reports.items()]
        self.ref("composition.json", self.bytes(self.manifest))
        return evaluate(self.root, self.sha, FOLDER + "/composition.json")

    def rule(self, rule):
        result = self.run_check()
        self.assertEqual(result["status"], "FAIL", result)
        self.assertIn(rule, result["findings"])

    def test_complete_consistency_fixture_preserves_112_61_and_never_decides_release(self):
        result = self.run_check()
        self.assertEqual(result["composition_checks"], "PASS", result)
        self.assertEqual(result["task_coverage"]["bound"], 111)
        self.assertEqual(result["ia_coverage"]["bound"], 61)
        self.assertEqual(result["qa08"], "DEFERRED_BY_USER")
        self.assertEqual(result["release_decision"], "NOT_ASSESSED")

    def test_missing_manifest_and_task_ia_records_remain_not_run(self):
        self.assertEqual(evaluate(self.root, self.sha)["composition_checks"], "NOT_RUN")
        self.manifest["tasks"] = []
        self.manifest["ia"] = []
        result = self.run_check()
        self.assertEqual(result["status"], "INCOMPLETE")
        self.assertEqual(len(result["task_coverage"]["missing"]), 112)
        self.assertEqual(len(result["ia_coverage"]["missing"]), 61)

    def test_qa08_cannot_pass_or_authorize_another_cut(self):
        self.manifest["deferrals"][0]["task_id"] = "QA09"
        self.rule("deferral.unauthorized-scope-cut")

    def test_qa08_pass_even_with_valid_deferral_is_rejected(self):
        next(t for t in self.manifest["tasks"] if t["id"] == "QA08")["status"] = "ACCEPTED"
        self.rule("QA08.cannot-pass")

    def test_missing_deferral_is_a_gap_not_an_implicit_pass(self):
        self.manifest["deferrals"] = []
        result = self.run_check()
        self.assertEqual(result["status"], "INCOMPLETE")
        self.assertIn("QA08.deferral.NOT_RUN", result["findings"])

    def test_mismatched_candidate_refuses_ancestor_receipt(self):
        self.reports["backend"]["candidate"] = {**self.candidate, "commit": "0" * 40}
        self.rule("candidate.mismatch")

    def test_dirty_current_source_refused(self):
        self.write(self.node, b"changed")
        self.rule("candidate.worktree-dirty")

    def test_final_candidate_fence_is_rechecked_after_evidence(self):
        original = source.Repository.git
        outer, calls = [], []
        def changed(repo, *args):
            if args == ("rev-parse", "HEAD"):
                if not outer:
                    outer.append(repo)
                if repo is outer[0]:
                    calls.append(True)
                    if len(calls) == 2:
                        return b"0000000000000000000000000000000000000000\n"
            return original(repo, *args)
        with patch.object(source.Repository, "git", changed):
            self.rule("candidate.end-changed")

    def test_no_recorded_command_is_executed_by_consumer(self):
        original = source.subprocess.run
        def git_only(argv, *args, **kwargs):
            self.assertEqual(argv[0], "git", "Receipt command must never execute")
            return original(argv, *args, **kwargs)
        with patch.object(source.subprocess, "run", git_only):
            self.assertEqual(self.run_check()["composition_checks"], "PASS")

    def test_failed_or_unexecuted_command_cannot_pass(self):
        self.reports["live"]["command"]["attempted"] = False
        self.rule("command.false-pass")

    def test_omitted_end_fence_fails(self):
        self.reports["backend"]["same_candidate_end"] = False
        self.rule("candidate.end-fence-missing")

    def test_unit_test_cannot_be_relabelled_live(self):
        self.reports["live"]["report"]["format"] = "tap"
        self.rule("evidence.unit-as-live")

    def test_live_context_requires_two_distinct_actual_actor_fingerprints(self):
        self.reports["live"]["context"]["actors"] = self.reports["live"]["context"]["actors"][:1]
        self.rule("context.actual-two-actors-required")

    def test_actor_observation_must_match_its_fingerprint(self):
        self.reports["live"]["context"]["actors"][0]["fingerprint"] = "33" * 32
        self.rule("context.observation-mismatch")

    def test_mock_and_synthetic_contexts_refused(self):
        self.reports["live"]["context"]["synthetic"] = True
        self.rule("context.mock-or-synthetic")

    def test_physical_device_is_not_emulation(self):
        self.reports["ios"]["context"]["device"]["physical"] = False
        self.rule("context.physical-device-required")

    def test_both_device_platforms_are_required_for_qa06(self):
        next(g for g in self.manifest["gates"] if g["id"] == "QA06")["receipt_ids"] = ["ios"]
        self.rule("gates.both-physical-platforms-required")

    def test_service_candidate_cannot_be_an_old_build(self):
        self.reports["live"]["context"]["services"][0]["candidate"]["commit"] = "0" * 40
        self.rule("candidate.mismatch")

    def test_missing_owner_evidence_is_not_qualified(self):
        self.reports["backend"] = {"schema": EVIDENCE_SCHEMA, "id": "backend", "candidate": self.candidate,
                                   "status": "BLOCKED", "owner": "fixture", "reason": "not provisioned"}
        self.manifest["tasks"] = []
        self.manifest["ia"] = []
        self.manifest["gates"] = []
        result = self.run_check()
        self.assertEqual(result["status"], "INCOMPLETE")
        self.assertIn("category.backend.NOT_RUN", result["findings"])

    def test_wrong_original_task_path_refused(self):
        self.manifest["tasks"][0]["paths"] = [self.native]
        self.rule("tasks.path-outside-original-task")

    def test_gate_requires_immutable_owner_report_coverage_not_a_manifest_label(self):
        self.reports["live"]["coverage"]["gate_ids"].remove("IN02")
        self.rule("binding.criteria-not-qualified")

    def test_partial_frontend_cannot_grant_live_ia_acceptance(self):
        self.manifest["ia"][0]["receipt_ids"] = ["frontend"]
        self.rule("binding.criteria-not-qualified")

    def test_unrelated_live_receipt_cannot_lend_its_level_to_native_ia_coverage(self):
        record = self.manifest["ia"][0]
        self.reports["live"]["coverage"]["ia_ids"].remove(record["id"])
        record["receipt_ids"] = ["backend", "live"]
        self.rule("ia.unit-as-product")

    def test_unrelated_live_receipt_cannot_lend_its_level_to_native_gate_coverage(self):
        record = next(row for row in self.manifest["gates"] if row["id"] == "IN02")
        self.reports["live"]["coverage"]["gate_ids"].remove("IN02")
        record["receipt_ids"] = ["backend", "live"]
        self.rule("gates.unit-as-live")

    def test_duplicate_or_unknown_original_id_refused(self):
        self.manifest["tasks"][0]["id"] = "INVENTED"
        self.rule("tasks.unknown-or-duplicate")

    def test_tampered_artifact_rejected(self):
        self.reports["backend"]["log"]["sha256"] = "0" * 64
        self.rule("artifact.hash-mismatch")

    def test_native_context_requires_isolated_actual_postgres(self):
        self.reports["backend"]["context"]["database"]["isolated"] = False
        self.rule("context.actual-database-required")

    def test_deployment_observation_must_match_actual_binary_record(self):
        self.reports["live"]["context"]["services"][0]["binary_sha256"] = "cd" * 32
        self.rule("context.observation-mismatch")

    def test_physical_device_observation_must_match_version_record(self):
        self.reports["ios"]["context"]["device"]["browser_version"] = "changed"
        self.rule("context.observation-mismatch")

    def test_fake_rust_zero_or_footer_only_results_refused(self):
        with self.assertRaises(source.Rejected):
            rust_counts(RUST.replace(b"test actual_native_scenario ... ok\n", b""))
        with self.assertRaises(source.Rejected):
            rust_counts(RUST.replace(b"1 passed", b"2 passed"))

    def test_rust_skip_is_retained_in_counts(self):
        value = RUST.replace(b"running 1 test", b"running 2 tests").replace(
            b"\ntest result:", b"\ntest skipped_case ... ignored\n\ntest result:").replace(b"0 ignored", b"1 ignored")
        self.assertEqual(rust_counts(value), {"total": 2, "passed": 1, "skipped": 1})

    def test_playwright_counts_refuse_retries_and_fabricated_stats(self):
        value = {"errors": [], "stats": {"expected": 1, "unexpected": 0, "flaky": 0, "skipped": 0},
                 "suites": [{"specs": [{"tests": [{"expectedStatus": "passed", "status": "expected",
                    "results": [{"status": "passed"}]}]}]}]}
        self.assertEqual(playwright_counts(self.bytes(value))["passed"], 1)
        value["stats"]["expected"] = 2
        with self.assertRaises(source.Rejected):
            playwright_counts(self.bytes(value))

    def test_unittest_requires_named_actual_points_not_just_ok_footer(self):
        text = b"test_boundary (module.Suite.test_boundary) ... ok\n\nRan 1 test in 0.001s\n\nOK\n"
        self.assertEqual(unittest_counts(text)["passed"], 1)
        with self.assertRaises(source.Rejected):
            unittest_counts(b"Ran 1 test in 0.001s\nOK\n")

    def test_default_source_and_explicit_composition_cli_stay_separate(self):
        self.run_check()
        executable = ROOT / "scripts/mcr/release_checks/check.py"
        for flags, key in [([], "source_checks"), (["--mode", "composition"], "composition_checks")]:
            output = io.StringIO()
            with patch.object(sys, "argv", [str(executable), "--root", str(self.root),
                                            "--candidate", self.sha, *flags]), contextlib.redirect_stdout(output):
                result = source.main()
            self.assertEqual(result, 2)
            self.assertEqual(json.loads(output.getvalue())[key], "NOT_RUN")


if __name__ == "__main__":
    unittest.main()
