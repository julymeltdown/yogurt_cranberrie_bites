"""Project the frozen public Meeting routes into the retained shared registry."""
import hashlib
import json
import subprocess

SOURCE = "0981774d48eab79c15666ef27612310d470b4f69"
PATH = "backend/docs/api/paradise-city-v1/meeting.openapi.json"
SHA256 = "b96a51257ac2b3c1535d520a188a3742e8aa51f018da41e50b535f02dfbd7bc0"


def extend(root, inputs, operations, queries, errors):
    local = root / PATH
    raw = local.read_bytes() if local.exists() else subprocess.check_output(
        ["git", "show", SOURCE + ":" + PATH], cwd=root)
    assert hashlib.sha256(raw).hexdigest() == SHA256, "Meeting frozen input drift"
    inputs[PATH] = SHA256
    api = json.loads(raw)

    def resolve(value):
        if "$ref" not in value:
            return value
        assert set(value) == {"$ref"}
        assert value["$ref"].startswith("#/components/")
        target = api
        for part in value["$ref"][2:].split("/"):
            target = target[part]
        return resolve(target)

    def scalar(value):
        value = resolve(value)
        allowed = {"type", "enum", "format", "pattern", "minimum", "maximum", "minLength", "maxLength", "not", "description"}
        assert set(value) <= allowed, "Unsupported Meeting scalar keyword"
        assert value.get("type") in (None, "string", "integer")
        if "not" in value:
            assert set(value["not"]) in ({"const"}, {"enum"})
        rule = {key: item for key, item in value.items() if key != "description"}
        if "pattern" in rule:
            # JS '$' permits a match immediately before a final newline. The
            # public canonical scalar grammar permits no trailing control byte.
            rule["pattern"] = "(?:" + rule["pattern"] + ")(?![\\s\\S])"
        return rule

    reasons = api["x-mcr-error-reasons"]
    for values in reasons.values():
        for reason in values:
            if reason not in errors:
                errors[reason] = None
    count = 0
    for path, methods in api["paths"].items():
        for method, operation in methods.items():
            name = operation["operationId"]
            assert name not in operations, "Duplicate Meeting operation"
            params = [resolve(item) for item in operation.get("parameters", [])]
            query = "paradiseMeeting." + name
            query_params = [item for item in params if item["in"] == "query"]
            queries[query] = {"required": [item["name"] for item in query_params if item.get("required")],
                              "properties": {item["name"]: scalar(item["schema"]) for item in query_params}}
            headers = [item for item in params if item["in"] == "header"]
            assert all(item["name"] == "Idempotency-Key" and item["required"] for item in headers)
            responses = operation["responses"]
            assert set(reasons) <= set(responses)
            operations[name] = {
                "method": method.upper(), "path": path, "query": query,
                "parameters": {item["name"]: scalar(item["schema"]) for item in params if item["in"] == "path"},
                "idempotency": bool(headers), "errors": reasons,
                "success": [int(status) for status in responses if status.isdecimal() and status.startswith("2")],
            }
            count += 1
    assert count == 8, "Meeting operation count drift"
