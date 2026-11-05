#!/usr/bin/env python3
"""Project candidate C04 transport metadata; this does not freeze or enable routes."""
import argparse
import hashlib
import json
import runpy
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "contracts/domain/v1"
DEST = ROOT / "frontend/src/shared/api/domain-operations.generated.ts"
inputs = {}


def read(path):
    raw = path.read_bytes()
    inputs[str(path.relative_to(ROOT))] = hashlib.sha256(raw).hexdigest()
    return json.loads(raw)


def resolve(schema):
    if "$ref" not in schema:
        return schema
    assert set(schema) == {"$ref"}, "Unsupported reference sibling"
    url, pointer = schema["$ref"].split("#")
    prefix = "https://contracts.mcr.invalid/"
    assert url.startswith(prefix) and pointer.startswith("/$defs/")
    path = (ROOT / "contracts" / url.removeprefix(prefix)).resolve()
    assert path.is_relative_to(ROOT / "contracts")
    return resolve(read(path)["$defs"][pointer.removeprefix("/$defs/")])


def scalar(schema):
    schema = resolve(schema)
    assert set(schema) <= {"type", "enum", "format", "pattern", "minimum", "maximum", "minLength", "maxLength", "default", "description"}, schema
    assert "description" not in schema or isinstance(schema["description"], str), "Invalid scalar description"
    assert schema.get("type") in {None, "string", "integer", "boolean"}
    assert schema.get("type") is not None or "enum" in schema
    assert schema.get("format") in {None, "uuid", "date-time"}
    return {k: v for k, v in schema.items() if k not in {"default", "description"}}


def query_schema(schema):
    assert {"type", "additionalProperties", "properties"} <= set(schema) <= {"type", "additionalProperties", "properties", "required"}
    assert schema["type"] == "object" and schema["additionalProperties"] is False
    assert isinstance(schema["properties"], dict)
    required = schema.get("required", [])
    assert isinstance(required, list) and all(isinstance(name, str) for name in required), "Invalid required list"
    assert len(required) == len(set(required)) and set(required) <= set(schema["properties"]), "Invalid required properties"
    return {"required": required, "properties": {k: scalar(v) for k, v in schema["properties"].items()}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--venue-only", action="store_true", help="Preserve the frozen existing registry; project only standalone venue additions")
    args = parser.parse_args()
    operations, queries, errors = {}, {}, {}
    for op in read(SOURCE / "operations.json"):
        assert op["operationId"] not in operations, "Duplicate operation"
        query = op["query_schema"]
        family, name = query.split(".")
        if query not in queries:
            schema = read(SOURCE / f"{family}.schema.json")["$defs"][name]
            queries[query] = query_schema(schema)
        operations[op["operationId"]] = {
            "method": op["method"].upper(), "path": op["path"], "query": query,
            "parameters": {k: scalar(v) for k, v in op["path_parameters"].items()},
            "idempotency": op["idempotency_key"], "errors": op["errors"],
            "success": [op["success_status"]] + ([op["pending_status"]] if op["pending_status"] else []),
        }
    for branch in read(SOURCE / "common.schema.json")["$defs"]["DomainError"]["properties"]["details"]["oneOf"]:
        props = branch["properties"]
        assert branch["additionalProperties"] is False
        assert set(props) in ({"reason"}, {"reason", "retry_after_ms"})
        assert "reason" in branch["required"] and set(branch["required"]) <= set(props)
        errors[props["reason"]["const"]] = ({
            **scalar(props["retry_after_ms"]),
            "required": "retry_after_ms" in branch["required"],
        } if "retry_after_ms" in props else None)
    # The retained generated registry includes previously delivered mural metadata
    # whose monolith inputs have not all been published together. Do not remove it.
    if args.venue_only:
        base = "217c52c25180217b9fd1264af9b8cae52ec88a1a"
        path = "frontend/src/shared/api/domain-operations.generated.ts"
        raw = subprocess.check_output(["git", "show", base + ":" + path], cwd=ROOT)
        assert hashlib.sha256(raw).hexdigest() == "3610456a0925aee9c4e3a32564016ea6acc26b7ab01ce536659b7333a5ddd447"
        text = raw.decode()
        def retained(name):
            block = text.split("export const " + name + " = {\n", 1)[1].split("\n} as const", 1)[0]
            return json.loads("{" + block.rstrip().removesuffix(",") + "}")
        operations, queries, errors = (retained(name) for name in ("DOMAIN_OPERATIONS", "DOMAIN_QUERIES", "DOMAIN_ERROR_RULES"))
        inputs.clear()
        inputs["git:" + base + ":" + path] = hashlib.sha256(raw).hexdigest()
    # Standalone qualified venue inputs are additive; historical C04 files stay unchanged.
    bridge = ROOT / "scripts/mcr/paradise-city-v1/place_notes_transport.py"
    inputs[str(bridge.relative_to(ROOT))] = hashlib.sha256(bridge.read_bytes()).hexdigest()
    runpy.run_path(str(bridge))["extend"](ROOT, inputs, operations, queries, errors)
    meeting = ROOT / "scripts/mcr/paradise-city-v1/meeting_transport.py"
    inputs[str(meeting.relative_to(ROOT))] = hashlib.sha256(meeting.read_bytes()).hexdigest()
    runpy.run_path(str(meeting))["extend"](ROOT, inputs, operations, queries, errors)
    location = ROOT / "scripts/mcr/paradise-city-v1/location_transport.py"
    inputs[str(location.relative_to(ROOT))] = hashlib.sha256(location.read_bytes()).hexdigest()
    runpy.run_path(str(location))["extend"](ROOT, inputs, operations, queries, errors)
    def emit(name, value):
        # One entry per line keeps repeated route metadata reviewable without inflating the bundle.
        rows = [f"  {json.dumps(k)}: {json.dumps(v, separators=(',', ':'))}," for k, v in value.items()]
        return f"export const {name} = {{\n" + "\n".join(rows) + "\n} as const\n"
    output = "// Generated by scripts/mcr/generate-domain-transport.py. C04 REVIEW_CANDIDATE, not route authority.\n"
    output += "// Inputs (SHA-256):\n" + "".join(f"// {k}: {v}\n" for k, v in sorted(inputs.items()))
    output += emit("DOMAIN_OPERATIONS", operations) + emit("DOMAIN_QUERIES", queries) + emit("DOMAIN_ERROR_RULES", errors)
    output += "export type DomainOperationId = keyof typeof DOMAIN_OPERATIONS\nexport type DomainReason = keyof typeof DOMAIN_ERROR_RULES\n"
    if args.check:
        assert DEST.read_text() == output, "Domain transport metadata drift; regenerate"
        print(f"Domain transport metadata current: {len(operations)} operations")
    else:
        DEST.write_text(output)
        print(f"Generated {DEST.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
