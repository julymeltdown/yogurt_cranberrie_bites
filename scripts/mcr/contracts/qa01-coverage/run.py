#!/usr/bin/env python3
"""QA01: fail-closed, offline coverage of canonical inputs and generated shapes."""
from __future__ import annotations

import argparse
from copy import deepcopy
import hashlib
import importlib.metadata
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(HERE))
from witness import Witnesses, canonical, nodes
from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource
from referencing.exceptions import NoSuchResource

FAMILIES = ("assets", "core", "domain", "realtime")
PIN = HERE / "pins.json"
METHODS = {"get", "post", "put", "patch", "delete", "options", "head", "trace"}
PROBES = ("broken-schema", "unknown-enum", "empty-index", "missing-fixture",
          "unindexed-fixture", "missing-operation", "missing-response", "drift",
          "broken-ref", "empty-responses", "empty-generated", "pinned-input-drift")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(path):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "duplicate JSON key: " + key)
            result[key] = value
        return result
    def nonfinite(value):
        raise ValueError("non-finite JSON: " + value)
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique,
                      parse_constant=nonfinite)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def relative(path):
    return path.relative_to(ROOT).as_posix()


def load(name, path):
    spec = importlib.util.spec_from_file_location("qa01_" + name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


# C07 is separately exhaustive in CI; no other namespace is excluded.
def legacy_path(path):
    name = path.relative_to(ROOT).as_posix()
    return not any(name.startswith(prefix) for prefix in (
        "contracts/fixtures/c07/", "contracts/generated/api-client/"))


def fixture_paths():
    files = {p for p in (ROOT / "contracts/fixtures").rglob("*") if p.is_file() and legacy_path(p)}
    json_files = {p for p in files if p.suffix == ".json"}
    require(files == json_files, "unclassified non-JSON fixture file")
    return sorted(json_files)


def inputs():
    paths = {p for p in (ROOT / "contracts").rglob("*.json") if legacy_path(p)}
    paths |= set((ROOT / "backend/scripts/docs").glob("*.py"))
    paths |= {ROOT / "scripts/mcr" / ("validate-" + family + ".py") for family in FAMILIES}
    paths |= {ROOT / "scripts/mcr/requirements-contracts.txt",
              ROOT / "scripts/mcr/generate-domain-transport.py",
              ROOT / "frontend/src/shared/api/domain-operations.generated.ts",
              ROOT / "backend/docs/api/mcr-core.openapi.json",
              ROOT / "backend/docs/api/mcr-domain.openapi.json"}
    require(all(path.is_file() for path in paths), "missing canonical input")
    return sorted(paths)


def operation_map(document):
    result = {}
    for path, methods in document.get("paths", {}).items():
        for method, operation in methods.items():
            if method not in METHODS:
                continue
            identity = operation["operationId"]
            require(identity not in result, "duplicate generated operation: " + identity)
            require(operation.get("responses"), "empty response coverage: " + identity)
            result[identity] = {"path": path, "method": method, "responses": operation["responses"]}
    require(result, "empty generated operation coverage")
    return result


def source_operations(registers, core_generator):
    result = {}
    inherited = core_generator.inherited_document()
    found = set()
    for path, methods in inherited["paths"].items():
        for method, op in methods.items():
            if op.get("operationId") in core_generator.AUTH_IDS:
                result[op["operationId"]] = ("/api" + path, method)
                found.add(op["operationId"])
    require(found == core_generator.AUTH_IDS, "missing inherited Auth operation")
    for family, rows in registers.items():
        require(rows, "empty operation register: " + family)
        for row in rows:
            identity = row["operationId"]
            require(identity not in result, "duplicate source operation: " + identity)
            result[identity] = (row["path"], row["method"])
    return result


def verify_operations(document, expected):
    actual = operation_map(document)
    require(set(actual) == set(expected), "missing/unexpected operation: " +
            str(sorted(set(actual) ^ set(expected))))
    for identity, (path, method) in expected.items():
        require((actual[identity]["path"], actual[identity]["method"]) == (path, method),
                "operation route drift: " + identity)


def verify_catalog(rows, files, family):
    require(isinstance(rows, list) and rows, "empty fixture index: " + family)
    declared = []
    for row in rows:
        name = row["file"]
        path = Path(name)
        flat = path.name == name and name.endswith(".json")
        nested = (family == "domain" and len(path.parts) == 2 and path.parts[0] == "wall-tools-b"
                  and path.as_posix() == name and "\\" not in name and path.name.endswith(".json"))
        require(flat or nested, "unsafe fixture path")
        if nested:
            actual = ROOT / "contracts/fixtures/domain" / path
            require(actual.resolve() == actual, "unsafe fixture path alias")
        declared.append(name)
    require(len(declared) == len(set(declared)), "duplicate indexed fixture")
    require(set(declared) == set(files), "unindexed/missing fixture: " + family)


def check_schemas(documents):
    require(documents, "empty source schema coverage")
    ids = [doc["$id"] for doc in documents]
    require(len(ids) == len(set(ids)), "duplicate schema ID")
    def offline(uri):
        raise NoSuchResource(ref=uri)
    registry = Registry(retrieve=offline).with_resources(
        (doc["$id"], Resource.from_contents(doc)) for doc in documents)
    for doc in documents:
        Draft202012Validator.check_schema(doc)
        for node in nodes(doc):
            if isinstance(node, dict) and "$ref" in node:
                registry.resolver(doc["$id"]).lookup(node["$ref"])
    return sum(len(doc.get("$defs", {})) for doc in documents)


def fixture_checks(modules, indexes, fixture_data, inventory):
    core, domain, realtime, assets = (modules[f] for f in ("core", "domain", "realtime", "assets"))
    core_doc, dc, rc = core.generator.build_document(), domain.Contract(), realtime.Contract()
    dc.check_inherited()
    for family in ("core", "domain", "realtime"):
        for row in indexes[family]:
            path = f"contracts/fixtures/{family}/{row['file']}"
            value = deepcopy(fixture_data[path])
            expected = row.get("expected", "accept")
            if family == "core":
                core.validate(value, row["schema"], core_doc, **{k: row.get(k) for k in (
                    "actor_id", "event_id", "target_user_id", "place_id", "operation_id", "http_status")})
            elif family == "domain":
                require(expected in ("accept", "reject"), "unknown fixture expectation")
                rejected = False
                try:
                    dc.validate(value, row["schema"])
                except domain.ContractError:
                    rejected = True
                require(rejected == (expected == "reject"), "fixture expectation mismatch: " + path)
            else:
                kind = row["kind"]
                if kind == "trace":
                    trace = realtime.Trace(rc, value["actor_id"])
                    for frame in value["messages"]:
                        trace.consume(frame["direction"], frame["message"])
                    require(sorted(trace.players) == value["expected_players"], "trace roster mismatch")
                elif kind == "projection":
                    realtime.filtered_projection(value, rc)
                elif kind == "wire-rejection":
                    expected = "reject"
                    try:
                        rc.decode(value["payload"], value["direction"], binary=value.get("binary", False))
                    except realtime.WireError as error:
                        require(error.code == value["close_code"], "wrong wire rejection code")
                    else:
                        raise ValueError("negative wire fixture accepted")
                elif kind == "http-response":
                    request = fixture_data[f"contracts/fixtures/realtime/{row['request_file']}"] if row.get("request_file") else None
                    rc.validate_http(value, row["operation_id"], row["status"], event_id=row.get("event_id"), request=request)
                else:
                    require(kind == "http", "unknown realtime fixture kind")
                    rc.validate(value, row["schema"])
            inventory.append({"file": path, "family": family, "expected": expected, "result": "PASS"})
    checks = assets.validators()
    for directory in ("contracts/fixtures/assets", "contracts/fixtures/assets/selected-static"):
        bundle = {Path(p).name: v for p, v in fixture_data.items() if str(Path(p).parent) == directory}
        assets.validate_bundle(bundle, checks)
        inventory.extend({"file": directory + "/" + name, "family": "assets", "expected": "accept", "result": "PASS"} for name in bundle)
    for path, value in fixture_data.items():
        if str(Path(path).parent) == "contracts/fixtures/assets/illustrated":
            negative = Path(path).name.startswith("invalid-")
            require(negative or Path(path).name == "valid-theme.json", "unclassified illustrated fixture")
            try:
                assets.validate_document(value, checks, path)
            except assets.AssetError:
                require(negative, "positive illustrated fixture rejected")
            else:
                require(not negative, "negative illustrated fixture accepted")
            inventory.append({"file": path, "family": "assets", "expected": "reject" if negative else "accept", "result": "PASS"})
    path = "contracts/fixtures/asset-semantics/illustrated-cases.json"
    cases = fixture_data[path]["cases"]
    require(cases and len({c["name"] for c in cases}) == len(cases), "empty/duplicate scenery cases")
    for case in cases:
        bundle = assets.load_bundle(assets.FIXTURES)
        theme = bundle["theme-revenge.json"]
        theme["scenery"] = deepcopy(case["scenery"])
        theme["content_hash"] = assets.content_hash(theme)
        rejected = False
        try:
            assets.validate_bundle(bundle, checks)
        except assets.AssetError:
            rejected = True
        require(rejected != case["valid"], "scenery case mismatch: " + case["name"])
    inventory.append({"file": path, "family": "asset-semantics", "cases": len(cases), "result": "PASS",
                      "case_inventory": [{"name": c["name"], "expected": "accept" if c["valid"] else "reject",
                                          "result": "PASS"} for c in cases]})
    motion = load("qa01_c07_motion", "scripts/mcr/contracts/c07/motion_coverage.py")
    motion.check_motion_fixtures(ROOT, read, fixture_data, inventory)
    covered = {entry["file"] for entry in inventory}
    actual = set(fixture_data) - {f"contracts/fixtures/{f}/index.json" for f in indexes}
    require(covered == actual, "unclassified fixture files: " + str(sorted(covered ^ actual)))
    return core_doc


def response_checks(document, validator_class, corpus):
    checker = FormatChecker()
    # These inherited OpenAPI formats are annotations, not wire validators.
    annotations = {"int64", "password", "binary"}
    def validator(schema):
        return validator_class({"components": document["components"], **schema}, format_checker=checker)
    for name, schema in document["components"]["schemas"].items():
        Draft202012Validator.check_schema(schema)
        for node in nodes(schema):
            if isinstance(node, dict) and isinstance(node.get("format"), str):
                require(node["format"] in checker.checkers or node["format"] in annotations, "missing format checker")
    witnesses = Witnesses(document, validator, corpus)
    inventory = []
    for identity, operation in sorted(operation_map(document).items()):
        for status, response in sorted(operation["responses"].items()):
            headers = {}
            for name, header in response.get("headers", {}).items():
                if "$ref" in header:
                    header = witnesses.resolve(header["$ref"])
                schema = header["schema"]
                Draft202012Validator.check_schema(schema)
                value = witnesses.make(schema)
                validator(schema).validate(value)
                headers[name] = {"schema_sha256": sha(canonical(schema).encode()), "witness": value}
            content = response.get("content", {})
            if not content:
                require(status in {"204", "302"}, "unclassified bodyless response: " + identity + " " + status)
                require(status != "302" or "Location" in response.get("headers", {}), "redirect without Location")
                inventory.append({"operation": identity, "status": status, "media": None, "headers": headers, "result": "BODYLESS_VERIFIED"})
            for media, target in sorted(content.items()):
                schema = target["schema"]
                Draft202012Validator.check_schema(schema)
                entry = {"operation": identity, "status": status, "media": media, "headers": headers,
                         "schema_sha256": sha(canonical(schema).encode())}
                if media.startswith("image/"):
                    require(schema == {"type": "string", "format": "binary"}, "binary response shape drift")
                    entry["result"] = "BINARY_SCHEMA_VERIFIED"
                else:
                    require(media in {"application/json", "text/html"}, "unclassified response media type: " + media)
                    try:
                        value = witnesses.make(schema)
                    except ValueError as error:
                        raise ValueError(f"{identity} {status} {media}: {error}") from error
                    validator(schema).validate(value)
                    entry.update(result="JSON_WITNESS_VALIDATED" if media == "application/json"
                                 else "HTML_STRING_SCHEMA_VALIDATED", witness=value)
                    if isinstance(value, dict) and isinstance(value.get("details"), dict) and "reason" in value["details"]:
                        unknown = deepcopy(value)
                        unknown["details"]["reason"] = "UNKNOWN_QA01_ENUM"
                        require(not validator(schema).is_valid(unknown), "unknown response reason accepted: " + identity)
                        entry["unknown_reason_probe"] = "REJECTED"
                inventory.append(entry)
    require(inventory, "empty generated response coverage")
    return inventory


def run(args, report):
    dependencies = {}
    for line in (ROOT / "scripts/mcr/requirements-contracts.txt").read_text().splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        name, version = line.strip().split("==")
        dependencies[name] = importlib.metadata.version(name)
        require(dependencies[name] == version, "contract dependency version mismatch: " + name)
    report["dependencies"] = dependencies
    paths = inputs()
    hashes = {relative(p): sha(p.read_bytes()) for p in paths}
    if args.probe == "pinned-input-drift":
        hashes["contracts/core/v1/core.schema.json"] = "0" * 64
    registers = {f: read(ROOT / f"contracts/{f}/v1/operations.json") for f in ("core", "domain", "realtime")}
    modules = {f: load(f, f"scripts/mcr/validate-{f}.py") for f in FAMILIES}
    core_gen = modules["core"].generator
    expected = source_operations(registers, core_gen)
    if args.freeze:
        require(not PIN.exists(), "pins already exist; do not silently repin")
        PIN.write_text(json.dumps({"canonical_candidate": git("rev-parse", "HEAD"), "inputs": hashes,
                                  "operations": {k: list(v) for k, v in sorted(expected.items())}}, indent=2) + "\n")
    pins = read(PIN)
    require(pins["inputs"] == hashes, "pinned input drift: " + str(sorted(k for k in set(hashes) | set(pins["inputs"]) if hashes.get(k) != pins["inputs"].get(k))))
    require(pins["operations"] == {k: list(v) for k, v in sorted(expected.items())}, "frozen operation drift")
    source_paths = [p for f in FAMILIES for p in sorted((ROOT / f"contracts/{f}/v1").glob("*.schema.json"))]
    source_paths.extend(ROOT / name for name in (
        "contracts/assets/v2/avatar-config.schema.json",
        "contracts/assets/v2/avatar-manifest.schema.json",
        "contracts/assets/v2/reviewed/avatar-config.schema.json",
        "contracts/assets/v2/reviewed/avatar-manifest.schema.json"))
    require(all(any(p.parent.parent.name == f for p in source_paths) for f in FAMILIES), "missing schema family")
    documents = [read(p) for p in source_paths]
    fixture_data = {relative(p): read(p) for p in fixture_paths()}
    indexes = {f: fixture_data[f"contracts/fixtures/{f}/index.json"] for f in ("core", "domain", "realtime")}
    document = read(ROOT / "contracts/generated/mcr-domain.openapi.json")
    if args.probe == "broken-schema": documents[0]["type"] = "BROKEN_TYPE"
    if args.probe == "broken-ref": documents[0]["$defs"]["QA01Broken"] = {"$ref": "https://contracts.mcr.invalid/missing"}
    if args.probe == "unknown-enum": fixture_data["contracts/fixtures/assets/selected-static/photo01_front_01.json"]["representation"] = "UNKNOWN_QA01"
    if args.probe == "empty-index": indexes["core"] = []
    if args.probe == "missing-fixture": del fixture_data["contracts/fixtures/core/auth-success.json"]
    if args.probe == "unindexed-fixture": fixture_data["contracts/fixtures/core/qa01-unindexed.json"] = {}
    first = next(iter(document["paths"].values()))
    method = next(k for k in first if k in METHODS)
    if args.probe == "missing-operation": del first[method]
    if args.probe == "missing-response": first[method]["responses"].pop(next(iter(first[method]["responses"])))
    if args.probe == "empty-responses": first[method]["responses"] = {}
    if args.probe == "empty-generated": document["paths"] = {}
    if args.probe == "drift": document["info"]["title"] += " DRIFT"
    report["schema_definitions"] = check_schemas(documents)
    for family, rows in indexes.items():
        prefix = f"contracts/fixtures/{family}"
        directories = {prefix} | ({prefix + "/wall-tools-b"} if family == "domain" else set())
        files = [Path(p).relative_to(prefix).as_posix() for p in fixture_data
                 if str(Path(p).parent) in directories and Path(p).name != "index.json"]
        verify_catalog(rows, files, family)
    verify_operations(document, expected)
    generator = load("domain_generator", "backend/scripts/docs/generate-mcr-domain.py")
    require(document == generator.build_document(), "generated OpenAPI drift")
    generator.validate_document(document)
    report["fixtures"] = []
    core_document = fixture_checks(modules, indexes, fixture_data, report["fixtures"])
    for path in ("contracts/generated/mcr-core.openapi.json", "backend/docs/api/mcr-core.openapi.json"):
        require(read(ROOT / path) == core_document, "generated core drift: " + path)
    require(read(ROOT / "backend/docs/api/mcr-domain.openapi.json") == document, "backend domain mirror drift")
    report["generators"] = []
    for path in ("backend/scripts/docs/generate-mcr-core.py", "backend/scripts/docs/generate-mcr-domain.py", "scripts/mcr/generate-domain-transport.py"):
        process = subprocess.run([sys.executable, "-B", path, "--check"], cwd=ROOT, text=True, capture_output=True)
        report["generators"].append({"command": f"python3 -B {path} --check", "exit": process.returncode, "output": (process.stdout + process.stderr).strip()})
        require(process.returncode == 0, "generator drift/check failure: " + path)
    corpus = list(fixture_data.values())
    report["responses"] = response_checks(document, modules["core"].CoreValidator, corpus)
    report.update(inputs=hashes, operations=len(expected), schema_files=len(source_paths),
                  generated_components=len(document["components"]["schemas"]),
                  fixture_files=len(fixture_data), fixture_indexes=len(indexes),
                  canonical_candidate=pins["canonical_candidate"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--freeze", action="store_true", help="create initial owned pins once; never updates an existing pin")
    modes.add_argument("--probe", choices=PROBES)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    report = {"task": "QA01", "result": "IN_PROGRESS", "candidate": git("rev-parse", "HEAD"),
              "probe": args.probe, "qualification": "OFFLINE_CONTRACT_COVERAGE_ONLY",
              "live_services": "NOT_RUN", "devices": "NOT_RUN", "IN_gates": "NOT_RUN",
              "harness_hashes": {relative(p): sha(p.read_bytes()) for p in sorted(HERE.glob("*.py"))},
              "pins_sha256": sha(PIN.read_bytes()) if PIN.exists() else None}
    try:
        run(args, report)
        report["result"] = "PASS"
        code = 0
    except Exception as error:
        report.update(result="FAIL", error=f"{type(error).__name__}: {error}")
        code = 1
    if args.output:
        destination = args.output.resolve()
        require(destination.is_relative_to(ROOT / "docs/mcr/evidence/contracts/qa01-coverage"), "report outside owned evidence scope")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({k: report[k] for k in ("task", "result", "candidate", "probe")}
                     | {"error": report.get("error"), "fixtures": len(report.get("fixtures", [])),
                        "responses": len(report.get("responses", [])), "operations": report.get("operations")}))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
