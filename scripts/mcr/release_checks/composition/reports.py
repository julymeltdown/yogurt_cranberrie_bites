"""Bounded owner-produced evidence readers. Never executes recorded commands."""
import re
from pathlib import PurePosixPath

import check as source

SCHEMA = "mcr.qa10.composition-evidence.v1"
LEVELS = {"unit", "native", "live", "device", "operator"}
CATEGORIES = {"backend", "frontend", "service", "device", "operator"}


def closed(value, keys, rule):
    source.require(isinstance(value, dict) and set(value) == set(keys), rule)


def names(value, rule):
    source.require(isinstance(value, list) and value and all(
        isinstance(v, str) and v and len(v) <= 256 for v in value)
        and len(set(value)) == len(value), rule)
    return value


def rust_counts(data):
    text = data.decode("utf-8")
    summaries = re.findall(r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; "
                           r"(\d+) ignored; (\d+) measured; (\d+) filtered out;.*$", text, re.M)
    source.require(len(summaries) == 1, "rust.summary")
    status, passed, failed, skipped, measured, _ = summaries[0]
    passed, failed, skipped, measured = map(int, (passed, failed, skipped, measured))
    cases = re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)(?:\s.*)?$", text, re.M)
    running = re.findall(r"^running (\d+) tests?$", text, re.M)
    source.require(len(running) == 1 and int(running[0]) == passed + failed + skipped
                   and len(cases) == int(running[0]) and measured == 0, "rust.inventory")
    source.require(len({n for n, _ in cases}) == len(cases), "rust.duplicate-case")
    source.require(sum(s == "ok" for _, s in cases) == passed
                   and sum(s == "FAILED" for _, s in cases) == failed
                   and sum(s == "ignored" for _, s in cases) == skipped, "rust.count-mismatch")
    source.require(status == "ok" and passed > 0 and failed == 0, "rust.failed-or-empty")
    return {"total": len(cases), "passed": passed, "skipped": skipped}


def playwright_counts(data, expected_files=None):
    report = source.decode(data)
    source.require(isinstance(report, dict) and not report.get("errors"), "playwright.errors")
    cases, files = [], set()

    def walk(suites):
        source.require(isinstance(suites, list), "playwright.suites")
        for suite in suites:
            source.require(isinstance(suite, dict), "playwright.suite")
            for spec in suite.get("specs", []):
                source.require(isinstance(spec, dict) and isinstance(spec.get("tests"), list)
                               and spec["tests"], "playwright.spec")
                if expected_files is not None:
                    file = spec.get("file", suite.get("file"))
                    source.require(isinstance(file, str), "playwright.file")
                    matches = [p for p in expected_files if file == p or file.endswith("/" + p)]
                    source.require(len(matches) == 1, "playwright.unexpected-file")
                    files.add(matches[0])
                for test in spec["tests"]:
                    source.require(test.get("expectedStatus") == "passed", "playwright.expected-failure")
                    results = test.get("results")
                    source.require(isinstance(results, list) and results, "playwright.empty-case")
                    states = [r.get("status") for r in results]
                    source.require(len(states) == 1 and states[0] in ("passed", "skipped"),
                                   "playwright.failed-or-retried")
                    source.require(test.get("status") == ("expected" if states[0] == "passed" else "skipped"),
                                   "playwright.status-mismatch")
                    cases.append(states[0])
            walk(suite.get("suites", []))

    walk(report.get("suites"))
    if expected_files is not None:
        source.require(files == set(expected_files), "playwright.file-inventory")
    stats = report.get("stats", {})
    passed, skipped = cases.count("passed"), cases.count("skipped")
    source.require(cases and passed > 0 and all(source.natural(stats.get(k))
                   for k in ("expected", "unexpected", "flaky", "skipped")), "playwright.counts")
    source.require(stats["expected"] == passed and stats["skipped"] == skipped
                   and stats["unexpected"] == stats["flaky"] == 0, "playwright.count-mismatch")
    return {"total": len(cases), "passed": passed, "skipped": skipped}


def observation_counts(repo, data, report, expected):
    doc = source.decode(data)
    closed(doc, ("schema", "candidate", "command", "cases"), "observations.shape")
    source.require(doc["schema"] == "mcr.qa10.observations.v1", "observations.schema")
    repo.candidate(doc["candidate"], expected)
    source.require(doc["command"] == report["command"], "observations.command-mismatch")
    cases = doc["cases"]
    source.require(isinstance(cases, list) and cases, "observations.empty")
    seen, passed, skipped = set(), 0, 0
    for case in cases:
        closed(case, ("id", "status", "artifacts"), "observations.case-shape")
        source.require(isinstance(case["id"], str) and case["id"] and case["id"] not in seen,
                       "observations.case-id")
        seen.add(case["id"])
        source.require(case["status"] in ("PASS", "NOT_RUN"), "observations.failed")
        source.require(isinstance(case["artifacts"], list) and case["artifacts"],
                       "observations.missing-artifact")
        for ref in case["artifacts"]:
            source.require(repo.artifact(ref), "observations.empty-artifact")
        passed += case["status"] == "PASS"
        skipped += case["status"] == "NOT_RUN"
    source.require(passed > 0, "observations.no-pass")
    return {"total": len(cases), "passed": passed, "skipped": skipped}


def bound_observation(repo, ref, expected, facts):
    doc = source.decode(repo.artifact(ref))
    closed(doc, ("schema", "candidate", "facts", "artifacts"), "context.observation-shape")
    source.require(doc["schema"] == "mcr.qa10.context-observation.v1", "context.observation-schema")
    repo.candidate(doc["candidate"], expected)
    source.require(doc["facts"] == facts, "context.observation-mismatch")
    source.require(isinstance(doc["artifacts"], list) and doc["artifacts"], "context.observation-empty")
    for artifact in doc["artifacts"]:
        source.require(repo.artifact(artifact), "context.observation-empty-artifact")


def context(repo, value, expected, level):
    """Validate explicit runtime observations, not an attestation of their truth."""
    closed(value, ("candidate", "mock_mode", "synthetic", "services", "actors", "database",
                   "device", "operator"), "context.shape")
    repo.candidate(value["candidate"], expected)
    source.require(value["mock_mode"] is False and value["synthetic"] is False,
                   "context.mock-or-synthetic")
    services = value["services"]
    source.require(isinstance(services, list), "context.services")
    seen = set()
    for service in services:
        closed(service, ("id", "candidate", "binary_sha256", "observation"), "context.service-shape")
        source.require(isinstance(service["id"], str) and service["id"]
                       and service["id"] not in seen, "context.service-id")
        seen.add(service["id"])
        repo.candidate(service["candidate"], expected)
        source.require(isinstance(service["binary_sha256"], str)
                       and source.HEX64.fullmatch(service["binary_sha256"]), "context.binary-pin")
        bound_observation(repo, service["observation"], expected,
                          {"kind": "service", "id": service["id"], "binary_sha256": service["binary_sha256"]})
    actors = value["actors"]
    source.require(isinstance(actors, list) and len(set(actors)) == len(actors)
                   and all(isinstance(a, str) and source.HEX64.fullmatch(a) for a in actors),
                   "context.actor-fingerprints")
    if level in ("live", "device", "operator"):
        source.require(services, "context.actual-services-required")
    if level == "live":
        source.require(len(actors) >= 2, "context.actual-two-actors-required")
    if level == "native":
        db = value["database"]
        closed(db, ("engine", "version", "isolated", "observation"), "context.database-shape")
        source.require(db["engine"] == "postgresql" and isinstance(db["version"], str)
                       and db["version"] and db["isolated"] is True, "context.actual-database-required")
        bound_observation(repo, db["observation"], expected,
                          {"kind": "database", **{k: v for k, v in db.items() if k != "observation"}})
    if level == "device":
        device = value["device"]
        closed(device, ("platform", "physical", "hardware", "os_version", "browser", "browser_version",
                       "widths", "themes", "observation"), "context.device-shape")
        source.require(device["platform"] in ("ios", "android") and device["physical"] is True,
                       "context.physical-device-required")
        source.require(all(isinstance(device[k], str) and device[k] for k in
                           ("hardware", "os_version", "browser", "browser_version")), "context.device-versions")
        source.require(device["browser"] == ("Safari" if device["platform"] == "ios" else "Chrome"),
                       "context.device-browser")
        source.require(sorted(device["widths"]) == [360, 390, 430]
                       and set(device["themes"]) == {"REVENGE", "PARADE", "DANGER"},
                       "context.device-coverage")
        bound_observation(repo, device["observation"], expected,
                          {"kind": "device", **{k: v for k, v in device.items() if k != "observation"}})
    if level == "operator":
        operator = value["operator"]
        closed(operator, ("deployment", "retention", "place_provenance", "incident_controls"),
               "context.operator-shape")
        for setting, ref in operator.items():
            bound_observation(repo, ref, expected, {"kind": "operator", "setting": setting, "status": "VERIFIED"})
    return value


def evidence(repo, report, expected, folder):
    if report.get("schema") == "mcr.qa10.frontend.v1":
        counts = source.check_frontend(repo, report, expected, folder)
        return {"category": "frontend", "level": "unit", "counts": counts,
                "pins": report["source_pins"], "context": None}
    closed(report, ("schema", "id", "candidate", "same_candidate_end", "status", "owner", "category",
                    "level", "command", "source_pins", "test_files", "log", "report", "context"),
           "evidence.shape")
    source.require(report["schema"] == SCHEMA and report["status"] == "PASS"
                   and isinstance(report["owner"], str) and report["owner"], "evidence.schema-or-owner")
    repo.candidate(report["candidate"], expected)
    source.require(report["same_candidate_end"] is True, "candidate.end-fence-missing")
    source.require(report["category"] in CATEGORIES and report["level"] in LEVELS,
                   "evidence.classification")
    source.require((report["category"] not in ("device", "operator")
                    or report["level"] == report["category"])
                   and (report["category"] != "service" or report["level"] == "live"),
                   "evidence.unit-as-live")
    source.command_provenance(report["command"])
    repo.pins(report["source_pins"], expected)
    source.require(repo.artifact(report["log"]), "evidence.empty-log")
    files = names(report["test_files"], "evidence.test-inventory")
    source.require(all(f in report["source_pins"] for f in files), "evidence.unpinned-test")
    closed(report["report"], ("format", "artifact"), "evidence.report-shape")
    data = repo.artifact(report["report"]["artifact"])
    fmt, argv, cwd = report["report"]["format"], report["command"]["argv"], report["command"]["cwd"]
    if fmt == "rust-libtest":
        source.require(PurePosixPath(argv[0]).name == "cargo" and "test" in argv,
                       "rust.runner-mismatch")
        source.require(all(f.endswith(".rs") and "--test" in argv
                           and PurePosixPath(f).stem == argv[argv.index("--test") + 1] for f in files),
                       "rust.not-dispatched")
        counts = rust_counts(data)
    elif fmt == "playwright-json":
        source.require(any("playwright" in a for a in argv) and "test" in argv, "playwright.runner")
        source.require(all(str(PurePosixPath(cwd) / a) in files for a in argv if a.endswith((".spec.ts", ".spec.js")))
                       and all(any(str(PurePosixPath(cwd) / a) == f for a in argv) for f in files),
                       "playwright.not-dispatched")
        counts = playwright_counts(data, files)
    elif fmt in ("tap", "vitest-json"):
        source.require(report["level"] == "unit", "evidence.unit-as-live")
        command = {**report, "schema": source.COMMAND_SCHEMA, "kind": "tests"}
        counts = source.check_command(repo, command, expected)
    else:
        source.require(fmt == "observations-json" and report["level"] in ("live", "device", "operator"),
                       "evidence.unsupported-format")
        dispatched = {str(PurePosixPath(cwd) / a) for a in argv if not a.startswith(("-", "/"))}
        source.require(set(files) <= dispatched, "observations.not-dispatched")
        # Observations bind the actual executed command exactly; never run it here.
        counts = observation_counts(repo, data, report, expected)
    observed = context(repo, report["context"], expected, report["level"])
    return {"category": report["category"], "level": report["level"], "counts": counts,
            "pins": report["source_pins"], "context": observed}
