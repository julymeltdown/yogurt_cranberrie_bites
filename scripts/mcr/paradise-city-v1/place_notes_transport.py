"""Add frozen N01 metadata without changing the historical C04 registry inputs."""
import hashlib
import json
import subprocess

SOURCE = "a378329c242f950f36b868ff50c9ef19bf6ed6c4"
PATH = "backend/docs/api/paradise-city-v1/place-notes.openapi.json"
SHA256 = "5b14c48c7b288ba4a1c7a68c5e0d84c16cc9805ae87e98b18c091957c2f24f1e"


def extend(root, inputs, operations, queries, errors):
    path = root / PATH
    raw = path.read_bytes() if path.exists() else subprocess.check_output(
        ["git", "show", SOURCE + ":" + PATH], cwd=root)
    assert hashlib.sha256(raw).hexdigest() == SHA256, "N01 frozen input drift"
    inputs[PATH] = SHA256
    api = json.loads(raw)
    reasons = api["components"]["schemas"]["NoteError"]["properties"]["details"]["properties"]["reason"]["enum"]
    for reason in reasons:
        if reason not in errors:
            errors[reason] = None
        else:
            assert errors[reason] is None, "Existing error rule conflict"
    for path, methods in api["paths"].items():
        for method, op in methods.items():
            name = op["operationId"]
            assert name not in operations, "Duplicate N01 operation"
            query = "paradiseNotes." + name
            params = op.get("parameters", [])
            def scalar(parameter):
                schema = parameter["schema"]
                allowed = {"type", "enum", "format", "pattern", "minimum", "maximum", "minLength", "maxLength", "default", "not"}
                assert set(schema) <= allowed, "Unsupported N01 scalar keyword"
                if "not" in schema:
                    assert set(schema["not"]) in ({"const"}, {"enum"})
                return {k: v for k, v in schema.items() if k != "default"}
            query_params = [p for p in params if p["in"] == "query"]
            queries[query] = {"required": [p["name"] for p in query_params if p.get("required")],
                              "properties": {p["name"]: scalar(p) for p in query_params}}
            headers = [p for p in params if p["in"] == "header"]
            assert all(p["name"] == "Idempotency-Key" and p["required"] for p in headers)
            operations[name] = {
                "method": method.upper(), "path": path, "query": query,
                "parameters": {p["name"]: scalar(p) for p in params if p["in"] == "path"},
                "idempotency": bool(headers),
                "errors": {s: reasons for s in op["responses"] if not s.startswith("2")},
                "success": [int(s) for s in op["responses"] if s.startswith("2")],
            }
