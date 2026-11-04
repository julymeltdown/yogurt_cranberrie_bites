#!/usr/bin/env python3
"""Offline QA10 evidence consistency checks. Never executes receipt commands."""
import argparse
import gzip
import io
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path, PurePosixPath

SCHEMA = "mcr.qa10.release-source.v1"
COMMAND_SCHEMA = "mcr.qa10.command.v1"
FRONTEND_GATES = {"dependencies", "python-formats", "typecheck", "lint", "vitest",
                  "node-tests", "build", "ui-origin", "production-static", "task-trace"}
LIVE_GATES = ("QA02-manual", "QA03", "QA04", "QA05", "QA06", "QA07", "QA08", "QA09",
              "IN01", "IN02", "IN03", "IN04", "IN05")
LIMIT = 32 * 1024 * 1024
VITEST_LIMIT = 96 * 1024 * 1024
HEX40 = re.compile(r"[0-9a-f]{40}\Z")
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
ALIASES = {"CD": "backend/crates/concert-domain/src", "CA": "backend/crates/concert-application/src",
           "CS": "backend/services/concert-service/src", "GW": "backend/services/api-gateway/src",
           "FS": "frontend/src", "CT": "contracts", "TEST": "backend/services/concert-service/tests"}


class Rejected(Exception):
    """Messages are fixed rule IDs, never raw source/log/secret content."""


def require(ok, rule):
    if not ok:
        raise Rejected(rule)


def digest(value):
    return hashlib.sha256(value).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "json.duplicate-key")
        result[key] = value
    return result


def decode(data):
    try:
        return json.loads(data, object_pairs_hook=unique_object,
                          parse_constant=lambda _: (_ for _ in ()).throw(Rejected("json.nonfinite")))
    except (ValueError, UnicodeError):
        raise Rejected("json.invalid") from None


def relative_path(value):
    require(isinstance(value, str) and value and "\\" not in value and "\0" not in value,
            "path.invalid")
    require(not value.startswith("/") and all(p not in ("", ".", "..", ".git")
            for p in value.split("/")), "path.unsafe")
    return value


def sensitive_path(path):
    return any(p.lower().startswith(".env") or p.lower() in ("secrets", "credentials", "credentials.json",
               ".ssh", ".aws", ".gnupg", "id_rsa", "id_ed25519")
               or p.lower().endswith((".pem", ".key", ".p12", ".pfx", ".jks"))
               for p in PurePosixPath(path).parts)


class Repository:
    def __init__(self, root):
        self.root = Path(root).resolve()
        self.read_bytes = 0
        self.trees = {}
        self.blobs = {}
        self.source_bytes = {}

    def git(self, *args):
        result = subprocess.run(["git", *args], cwd=self.root, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, timeout=30, check=False)
        require(result.returncode == 0, "git.operation-failed")
        require(len(result.stdout) <= LIMIT, "git.output-limit")
        return result.stdout

    def path(self, name):
        relative_path(name)
        require(not sensitive_path(name), "path.secret-input-refused")
        p = self.root
        for part in name.split("/"):
            p = p / part
            require(not p.is_symlink(), "path.symlink-refused")
        return p

    def read(self, name, limit=LIMIT):
        p = self.path(name)
        require(p.is_file() and p.stat().st_size <= limit, "artifact.missing-or-too-large")
        data = p.read_bytes()
        self.read_bytes += len(data)
        require(self.read_bytes <= 128 * 1024 * 1024, "artifact.total-read-limit")
        return data

    def artifact(self, ref):
        require(isinstance(ref, dict) and set(ref) == {"path", "sha256"}, "artifact.reference-shape")
        require(isinstance(ref["sha256"], str) and HEX64.fullmatch(ref["sha256"]), "artifact.hash-shape")
        data = self.read(ref["path"])
        require(digest(data) == ref["sha256"], "artifact.hash-mismatch")
        return data

    def candidate(self, value, expected):
        require(isinstance(value, dict) and value.get("commit") == expected, "candidate.mismatch")
        require(value.get("dirty") is False, "candidate.dirty")
        require(value.get("tree") == self.git("rev-parse", expected + "^{tree}").decode().strip(),
                "candidate.tree-mismatch")

    def pins(self, pins, expected):
        require(isinstance(pins, dict) and pins, "source.empty-pins")
        if expected not in self.trees:
            self.trees[expected] = {}
            for row in self.git("ls-tree", "-r", "-z", expected).split(b"\0"):
                if row:
                    meta, name = row.split(b"\t", 1)
                    mode, kind, blob = meta.decode().split()
                    self.trees[expected][name.decode()] = (mode, kind, blob)
        missing = set()
        for path, pin in pins.items():
            self.path(path)  # Refuse secret names and symlinks before any Git content read.
            require(isinstance(pin, dict) and set(pin) == {"git_blob", "sha256"}, "source.pin-shape")
            entry = self.trees[expected].get(path)
            require(entry and entry[:2] in (("100644", "blob"), ("100755", "blob"))
                    and entry[2] == pin["git_blob"],
                    "source.blob-mismatch")
            missing.add(entry[2])
        missing -= self.blobs.keys()
        if missing:
            child = subprocess.run(["git", "cat-file", "--batch"], cwd=self.root,
                                   input=("\n".join(sorted(missing)) + "\n").encode(),
                                   stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=30, check=False)
            require(child.returncode == 0 and len(child.stdout) <= 128 * 1024 * 1024, "source.batch-limit")
            stream = io.BytesIO(child.stdout)
            for blob in sorted(missing):
                header = stream.readline().decode().strip().split()
                require(len(header) == 3 and header[:2] == [blob, "blob"], "source.batch-shape")
                size = int(header[2])
                require(size <= LIMIT, "source.blob-limit")
                value = stream.read(size)
                require(len(value) == size and stream.read(1) == b"\n", "source.batch-truncated")
                self.blobs[blob] = value
        for pin in pins.values():
            require(digest(self.blobs[pin["git_blob"]]) == pin["sha256"], "source.hash-mismatch")
        for path, pin in pins.items():
            self.source_bytes[path] = self.blobs[pin["git_blob"]]


def natural(value):
    return type(value) is int and value >= 0


def tap_counts(data):
    text = data.decode("utf-8")
    require("Bail out!" not in text, "tests.tap-bailout")
    counts = {}
    for key in ("tests", "suites", "pass", "fail", "cancelled", "skipped", "todo"):
        values = re.findall(r"^# " + key + r" (\d+)$", text, re.M)
        require(len(values) == 1, "tests.tap-footer")
        counts[key] = int(values[0])
    points = re.findall(r"^\s*(?:not )?ok \d+(?:\s|$).*$", text, re.M)
    plan = re.findall(r"^1\.\.(\d+)$", text, re.M)
    top = re.findall(r"^(?:not )?ok (\d+)(?:\s|$)", text, re.M)
    require(len(plan) == 1 and list(map(int, top)) == list(range(1, int(plan[0]) + 1)),
            "tests.tap-plan")
    require(counts["tests"] > 0 and len(points) == counts["tests"] + counts["suites"],
            "tests.tap-empty-or-fabricated-count")
    require(sum(counts[k] for k in ("pass", "fail", "cancelled", "skipped", "todo")) == counts["tests"],
            "tests.tap-count-mismatch")
    require(not counts["fail"] and not counts["cancelled"] and counts["pass"] > 0
            and not re.search(r"^\s*not ok ", text, re.M), "tests.tap-failed")
    require(sum(bool(re.search(r"#\s*SKIP\b", p, re.I)) for p in points) == counts["skipped"],
            "tests.tap-skip-mismatch")
    require(sum(bool(re.search(r"#\s*TODO\b", p, re.I)) for p in points) == counts["todo"],
            "tests.tap-todo-mismatch")
    return {"total": counts["tests"], "passed": counts["pass"],
            "skipped": counts["skipped"] + counts["todo"]}


def vitest_counts(data, expected_files):
    report = decode(data)
    require(isinstance(report, dict), "tests.vitest-shape")
    require(all(natural(report.get(k, 0)) for k in ("numTotalTests", "numPassedTests", "numFailedTests",
                                                   "numPendingTests")), "tests.vitest-count-shape")
    results = report.get("testResults", [])
    require(isinstance(results, list) and results, "tests.vitest-empty")
    files, statuses = [], []
    for result in results:
        require(isinstance(result, dict) and isinstance(result.get("name"), str), "tests.vitest-file")
        require(result.get("status", "passed") == "passed", "tests.vitest-file-not-passing")
        name = result["name"].replace("\\", "/")
        matches = [p for p in expected_files if name == p or name.endswith("/" + p)]
        require(len(matches) == 1, "tests.vitest-unexpected-file")
        files.append(matches[0])
        assertions = result.get("assertionResults")
        require(isinstance(assertions, list) and assertions, "tests.vitest-empty-file")
        statuses.extend(a.get("status") for a in assertions)
    require(sorted(files) == sorted(expected_files), "tests.vitest-file-inventory")
    require(all(s in ("passed", "failed", "pending", "todo", "skipped") for s in statuses),
            "tests.vitest-unknown-status")
    total, passed, failed = len(statuses), statuses.count("passed"), statuses.count("failed")
    skipped = total - passed - failed
    require(report.get("numTotalTests") == total and report.get("numPassedTests") == passed
            and report.get("numFailedTests") == failed and report.get("numPendingTests", 0) == skipped,
            "tests.vitest-fabricated-count")
    require(report.get("success") is True and passed > 0 and failed == 0, "tests.vitest-failed")
    require(report.get("numFailedTestSuites", 0) == 0 and report.get("numRuntimeErrorTestSuites", 0) == 0,
            "tests.vitest-suite-failed")
    return {"total": total, "passed": passed, "skipped": skipped}


def command_provenance(command):
    require(isinstance(command, dict) and command.get("attempted") is True
            and type(command.get("exit_code")) is int and command["exit_code"] == 0
            and command.get("signal") is None, "command.false-pass")
    require(isinstance(command.get("argv"), list) and command["argv"]
            and all(isinstance(a, str) and a for a in command["argv"]), "command.missing-argv")
    require(command.get("cwd") == "." or relative_path(command.get("cwd")), "command.cwd")


def production_findings(modules, assets):
    require(isinstance(modules, list) and modules and len(set(modules)) == len(modules),
            "production.empty-or-duplicate-modules")
    findings = []
    for path in modules:
        require(isinstance(path, str), "production.module-shape")
        if re.search(r"(?:^|[/\\])(?:__tests__|fixtures?|e2e|mocks?|msw)(?:[/\\.]|$)", path, re.I):
            findings.append("production.mock-module")
    patterns = {
        "production.private-key": rb"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
        "production.jwt-literal": rb"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
        "production.credential-literal": rb"(?i)\b(?:access_?token|refresh_?token|password|client_secret|api_?key)\s*[:=]\s*[\"'][^\"'\r\n]{12,}[\"']",
        "production.mock-marker": rb"\b(?:MockWorldTransport|setupWorker|mockServiceWorker)\b",
    }
    for data in assets:
        findings.extend(rule for rule, pattern in patterns.items() if re.search(pattern, data))
    return sorted(set(findings))


def check_command(repo, report, expected):
    require(report.get("schema") == COMMAND_SCHEMA, "receipt.schema")
    repo.candidate(report.get("candidate"), expected)
    require(report.get("same_candidate_end") is True, "candidate.end-fence-missing")
    require(report.get("kind") in ("tests", "source", "build"), "receipt.unsupported-kind")
    require(report.get("status") == "PASS", "receipt.not-passing")
    command_provenance(report.get("command"))
    repo.pins(report.get("source_pins"), expected)
    repo.artifact(report.get("log"))
    evidence = report.get("report")
    require(isinstance(evidence, dict), "tests.report-missing")
    data = repo.artifact(evidence.get("artifact"))
    if report["kind"] == "tests":
        files = report.get("test_files")
        require(isinstance(files, list) and files and len(set(files)) == len(files)
                and all(p in report["source_pins"] for p in files), "tests.source-inventory")
        argv = report["command"]["argv"]
        cwd = report["command"]["cwd"]
        dispatched = {str(PurePosixPath(cwd) / a) for a in argv if not a.startswith(("-", "/"))}
        require(set(files) <= dispatched, "tests.not-dispatched")
        if evidence.get("format") == "tap":
            require("--test" in argv and PurePosixPath(argv[0]).name in ("node", "nodejs"),
                    "tests.runner-mismatch")
            return tap_counts(data)
        require(evidence.get("format") == "vitest-json" and any("vitest" in a for a in argv),
                "tests.runner-mismatch")
        return vitest_counts(data, files)
    require(evidence.get("format") == "execution-json", "command.report-format")
    execution = decode(data)
    repo.candidate(execution.get("candidate"), expected)
    require(execution.get("status") == "PASS", "command.report-not-passing")
    command_provenance(execution.get("command"))
    require(execution["command"] == report["command"], "command.report-mismatch")
    return {"total": 0, "passed": 0, "skipped": 0, "production": execution.get("production")}


def check_frontend(repo, report, expected, folder):
    """Consume existing frontend.v1 without running Node or accepting its global status."""
    repo.candidate(report.get("candidate"), expected)
    require(report.get("same_candidate_end") is True, "candidate.end-fence-missing")
    require(report.get("status") == "PARTIAL" and report.get("unsupported"), "frontend.false-acceptance")
    commands = report.get("commands", [])
    require(len(commands) == len(FRONTEND_GATES)
            and {c.get("id") for c in commands} == FRONTEND_GATES, "frontend.gate-inventory")
    for key in ("source_pins", "contract_pins"):
        repo.pins(report.get(key), expected)
    pins = report.get("artifact_pins")
    require(isinstance(pins, dict) and pins, "frontend.empty-artifacts")
    data = {}
    for path, sha in pins.items():
        relative_path(path)
        name = folder + "/" + path
        limit = VITEST_LIMIT if path == "vitest.json" else LIMIT
        if not repo.path(name).exists() and path.endswith(".json"):
            packed = repo.read(name + ".gz")
            try:
                with gzip.GzipFile(fileobj=io.BytesIO(packed)) as stream:
                    value = stream.read(limit + 1)
            except (OSError, EOFError):
                raise Rejected("frontend.invalid-gzip") from None
            require(len(value) <= limit, "frontend.archive-too-large")
            repo.read_bytes += len(value)
            require(repo.read_bytes <= 128 * 1024 * 1024, "artifact.total-read-limit")
        else:
            value = repo.read(name, limit=limit)
        require(digest(value) == sha, "artifact.hash-mismatch")
        data[path] = value
    by_id = {c["id"]: c for c in commands}
    for command in commands:
        require(command.get("status") == "PASS" and natural(command.get("nonempty_count"))
                and command["nonempty_count"] > 0, "frontend.false-pass")
        command_provenance(command)
        require(command.get("log") in data and digest(data[command["log"]]) == command.get("report_hash"),
                "frontend.command-log-pin")
    inventory = report.get("test_inventory", {})
    require(not inventory.get("unsupported") and inventory.get("vitest") and inventory.get("node"),
            "frontend.test-inventory")
    require(decode(data.get("test-inventory.json", b"{}")) == inventory, "frontend.inventory-pin-mismatch")
    discovered = {"vitest": [], "node": [], "playwright": [], "unsupported": []}
    for path in repo.trees[expected]:
        if not path.startswith("frontend/") or not re.search(r"\.[cm]?[jt]sx?$", path):
            continue
        require(path in repo.source_bytes, "frontend.missing-candidate-source-pin")
        text = repo.source_bytes[path].decode("utf-8")
        if re.search(r"(?:from\s*|import\s*\()['\"]node:test['\"]", text):
            discovered["node"].append(path)
        elif re.search(r"\.(?:test|spec|node-test)\.[cm]?[jt]sx?$", path):
            group = "vitest" if re.search(r"['\"]vitest['\"]", text) else "playwright" if re.search(
                r"['\"]@playwright/test['\"]", text) else "unsupported"
            discovered[group].append(path)
    require(all(sorted(inventory.get(k, [])) == sorted(v) for k, v in discovered.items()),
            "frontend.candidate-test-inventory")
    for group, gate in (("vitest", "vitest"), ("node", "node-tests")):
        files = inventory[group]
        require(len(set(files)) == len(files) and all(p in report["source_pins"] for p in files),
                "tests.source-inventory")
        command = by_id[gate]
        dispatched = {str(PurePosixPath(command["cwd"]) / a) for a in command["argv"]
                      if not a.startswith(("-", "/"))}
        require(set(files) <= dispatched, "tests.not-dispatched")
    counts = vitest_counts(data.get("vitest.json", b"{}"), inventory["vitest"])
    node = tap_counts(data.get("node-tests.log", b""))
    require(by_id["vitest"]["nonempty_count"] == counts["total"]
            and by_id["node-tests"]["nonempty_count"] == node["total"], "frontend.test-count-mismatch")
    audit = decode(data.get("build-audit.json", b"{}"))
    require(audit.get("assets") and audit.get("findings") == [], "frontend.build-audit-missing")
    require(not production_findings(audit.get("modules"), []), "production.mock-module")
    # Assets were removed by the existing producer. Hash-only inventories cannot be rescanned.
    return {"total": counts["total"] + node["total"], "passed": counts["passed"] + node["passed"],
            "skipped": counts["skipped"] + node["skipped"], "production_bytes": "NOT_RUN"}


def check_production(repo, production, receipts):
    require(isinstance(production, dict) and production.get("build_receipt") in receipts,
            "production.build-receipt-missing")
    require(receipts[production["build_receipt"]]["kind"] == "build", "production.not-build-receipt")
    require({k: v for k, v in production.items() if k != "build_receipt"}
            == receipts[production["build_receipt"]]["counts"].get("production"), "production.build-report-mismatch")
    root = relative_path(production.get("root"))
    # Only retained release outputs; never scan a source tree, credentials directory or home.
    require(root.startswith("docs/mcr/evidence/release/"), "production.output-root")
    directory = repo.path(root)
    require(directory.is_dir(), "production.output-missing")
    refs = production.get("assets")
    require(isinstance(refs, list) and refs, "production.empty-assets")
    listed = [r.get("path") for r in refs]
    require(len(set(listed)) == len(listed) and all(isinstance(p, str) and p.startswith(root + "/")
            for p in listed), "production.asset-inventory")
    actual = []
    for p in directory.rglob("*"):
        require(not p.is_symlink(), "path.symlink-refused")
        if p.is_file():
            actual.append(p.relative_to(repo.root).as_posix())
    require(sorted(actual) == sorted(listed), "production.omitted-assets")
    values = [repo.artifact(ref) for ref in refs]
    findings = production_findings(production.get("modules"), values)
    require(not findings, findings[0] if findings else "production.findings")
    return len(refs)


def evaluate(root, expected, manifest=None):
    result = {"schema": SCHEMA, "candidate": expected, "status": "INCOMPLETE",
              "source_checks": "NOT_RUN", "release_status": "INCOMPLETE", "findings": [],
              "receipts": [], "task_links": [], "missing_required_tasks": [],
              "live_gates": [{"id": gate, "status": "NOT_RUN"} for gate in LIVE_GATES],
              "limits": ["Offline consistency checks are not signed execution attestations or release acceptance.",
                         "No command from evidence is executed; live/device/operator and full secret audit remain unqualified."]}
    try:
        repo = Repository(root)
        require(isinstance(expected, str) and HEX40.fullmatch(expected), "candidate.full-sha-required")
        require(repo.git("rev-parse", "HEAD").decode().strip() == expected, "candidate.not-current-head")
        require(not repo.git("status", "--porcelain", "--untracked-files=no").strip(), "candidate.worktree-dirty")
        result["candidate_tree"] = repo.git("rev-parse", expected + "^{tree}").decode().strip()
        # Read the canonical planning catalog from the named candidate, never a mutable ledger.
        plan_bytes = repo.git("show", expected + ":generated/TASKS_EN.json")
        plan = decode(plan_bytes)
        tasks = {row["id"]: row for row in plan["tasks"]}
        required = {key for key, row in tasks.items() if row["priority"] == "P0"}
        result["canonical_plan"] = {"path": "generated/TASKS_EN.json", "sha256": digest(plan_bytes),
                                    "required_tasks": len(required)}
        result["missing_required_tasks"] = sorted(required)
        if manifest is None:
            result["findings"].append("manifest.NOT_RUN")
            return result
        manifest_bytes = repo.read(manifest)
        result["input_manifest"] = {"path": manifest, "sha256": digest(manifest_bytes)}
        document = decode(manifest_bytes)
        require(document.get("schema") == SCHEMA, "manifest.schema")
        require(set(document) <= {"schema", "candidate", "release_status", "receipts", "production", "task_links"},
                "manifest.unknown-field")
        repo.candidate(document.get("candidate"), expected)
        require(document.get("release_status", "INCOMPLETE") == "INCOMPLETE", "manifest.false-release-pass")
        refs = document.get("receipts", [])
        require(isinstance(refs, list), "manifest.receipt-shape")
        receipts = {}
        pending = []
        for ref in refs:
            report = decode(repo.artifact(ref))
            identity = report.get("id", "frontend" if report.get("schema") == "mcr.qa10.frontend.v1" else None)
            require(isinstance(identity, str) and re.fullmatch(r"[a-zA-Z0-9_-]+", identity)
                    and identity not in receipts and identity not in pending, "receipt.duplicate-or-invalid-id")
            if report.get("status") in ("NOT_RUN", "BLOCKED"):
                require(report.get("reason") and report.get("owner"), "receipt.missing-gap-reason")
                pending.append(identity)
                result["receipts"].append({"id": identity, "status": report["status"]})
                continue
            if report.get("schema") == "mcr.qa10.frontend.v1":
                counts = check_frontend(repo, report, expected, str(PurePosixPath(ref["path"]).parent))
                kind = "frontend"
            else:
                counts = check_command(repo, report, expected)
                kind = report["kind"]
            receipts[identity] = {"kind": kind, "pins": report.get("source_pins", {}), "counts": counts}
            result["receipts"].append({"id": identity, "status": "PASS",
                                       "counts": {k: v for k, v in counts.items() if k != "production"},
                                       "evidence": ref, "source_pins_verified": len(report.get("source_pins", {}))})
        links = document.get("task_links", [])
        require(isinstance(links, list), "trace.shape")
        seen = set()
        for link in links:
            task = link.get("task_id")
            require(task in tasks and task not in seen, "trace.unknown-or-duplicate-task")
            seen.add(task)
            commit = link.get("commit")
            require(isinstance(commit, str) and HEX40.fullmatch(commit), "trace.commit-shape")
            repo.git("merge-base", "--is-ancestor", commit, expected)
            paths, checks = link.get("paths"), link.get("receipt_ids")
            require(isinstance(paths, list) and paths and len(set(paths)) == len(paths)
                    and isinstance(checks, list) and checks and all(c in receipts for c in checks),
                    "trace.missing-source-or-evidence")
            for path in paths:
                repo.path(path)
                owned = []
                for pattern in tasks[task]["owned_paths"]:
                    first, separator, tail = pattern.partition("/")
                    owned.append(ALIASES.get(first, first) + (separator + tail if separator else ""))
                require(any(path == p or (p.endswith("/") and path.startswith(p)) for p in owned),
                        "trace.path-outside-task")
                require(any(path in receipts[c]["pins"] for c in checks), "trace.source-not-in-receipt")
                require(repo.git("rev-parse", commit + ":" + path) == repo.git("rev-parse", expected + ":" + path),
                        "trace.stale-source-commit")
            result["task_links"].append({"task_id": task, "commit": commit, "paths": paths,
                                         "receipt_ids": checks, "status": "SOURCE_LINKED", "acceptance": "NOT_ASSESSED"})
        result["missing_required_tasks"] = sorted(required - seen)
        gaps = []
        if pending:
            gaps.append("receipts.NOT_RUN-or-BLOCKED")
        if not receipts:
            gaps.append("receipts.NOT_RUN")
        if not any(r["kind"] in ("tests", "frontend") for r in receipts.values()):
            gaps.append("tests.NOT_RUN")
        if not links:
            gaps.append("trace.NOT_RUN")
        if document.get("production") is None:
            gaps.append("production.NOT_RUN")
        else:
            result["production_assets_checked"] = check_production(repo, document["production"], receipts)
        if any(r["counts"]["skipped"] for r in receipts.values()):
            gaps.append("tests.skipped-cases")
        result["findings"].extend(gaps)
        result["source_checks"] = "INCOMPLETE" if gaps else "PASS"
        result["status"] = "INCOMPLETE" if gaps else "PARTIAL"
        require(repo.git("rev-parse", "HEAD").decode().strip() == expected
                and not repo.git("status", "--porcelain", "--untracked-files=no").strip(), "candidate.end-changed")
    except Rejected as error:
        result["status"] = result["source_checks"] = "FAIL"
        result["findings"].append(str(error))
    except (OSError, KeyError, TypeError, ValueError, AttributeError, subprocess.TimeoutExpired):
        result["status"] = result["source_checks"] = "FAIL"
        result["findings"].append("input.malformed-or-unavailable")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--manifest", help="Repository-relative explicit sanitized evidence manifest")
    args = parser.parse_args()
    report = evaluate(args.root, args.candidate, args.manifest)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 1 if report["status"] == "FAIL" else 0 if report["source_checks"] == "PASS" else 2


if __name__ == "__main__":
    sys.exit(main())
