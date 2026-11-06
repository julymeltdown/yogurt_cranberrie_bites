"""Add pinned public Location metadata without replacing shared registry entries."""
import hashlib
import json
import subprocess

SOURCE = "4948e72a05a3cca05fd4fb85db959af7f0c5584f"
PATH = "backend/docs/api/paradise-city-v1/location.openapi.json"
SHA256 = "acbd0feaa10bdc742c8ce906050bf6a463b037f4a52c2c2e151827e78cefa32e"
OPERATIONS = {
    "getVenueLocationSnapshot": ("GET", "/location"),
    "setVenueLocationSharing": ("PUT", "/location/sharing"),
    "publishVenueLocationFix": ("PUT", "/location/fix"),
    "requestVenueLocation": ("POST", "/location/requests"),
    "decideVenueLocationRequest": ("POST", "/location/requests/{requestId}/decision"),
    "setVenueLocationCheckIn": ("PUT", "/location/check-in"),
    "getVenueLocationCommand": ("GET", "/location/commands/{key}"),
    "listVenueLandmarks": ("GET", "/landmarks"),
    "getVenueLandmark": ("GET", "/landmarks/{landmarkId}"),
}


def extend(root, inputs, operations, queries, errors):
    """Validate the complete projection before mutating any caller dictionary."""
    local = root / PATH
    raw = local.read_bytes() if local.exists() else subprocess.check_output(
        ["git", "show", SOURCE + ":" + PATH], cwd=root)
    if hashlib.sha256(raw).hexdigest() != SHA256:
        raise ValueError("Location frozen input drift")
    api = json.loads(raw)

    def resolve(value):
        if "$ref" not in value:
            return value
        if set(value) != {"$ref"} or not value["$ref"].startswith("#/components/schemas/"):
            raise ValueError("Unsupported Location reference")
        return resolve(api["components"]["schemas"][value["$ref"].removeprefix("#/components/schemas/")])

    def scalar(value):
        value = resolve(value)
        allowed = {"type", "enum", "format", "pattern", "minimum", "maximum", "minLength", "maxLength", "description"}
        if not set(value) <= allowed or value.get("type") not in {"string", "integer"}:
            raise ValueError("Unsupported Location scalar")
        result = {key: item for key, item in value.items() if key != "description"}
        if result.get("format") == "uuid":
            result["pattern"] = r"^(?!00000000-0000-0000-0000-000000000000$)[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$(?![\s\S])"
        elif "pattern" in result:
            # JavaScript '$' alone also matches before a final newline.
            result["pattern"] += r"(?![\s\S])"
        return result

    reason_values = api["components"]["schemas"]["LocationError"]["properties"]["details"]["properties"]["reason"]["enum"]
    # The frozen API references the same closed error schema for each status.
    # Preserve that contract instead of inventing a narrower reason/status table.
    reasons = {status: list(reason_values) for status in ["400", "401", "404", "409", "429", "503"]}
    projected, query_rules = {}, {}
    prefix = "/api/concert/events/{eventId}/venue"
    for path, methods in api["paths"].items():
        for method, operation in methods.items():
            name = operation["operationId"]
            if name not in OPERATIONS or (method.upper(), path.removeprefix(prefix)) != OPERATIONS[name]:
                raise ValueError("Location operation drift")
            params = [resolve(item) for item in operation.get("parameters", [])]
            query = "paradiseLocation." + name
            query_params = [item for item in params if item["in"] == "query"]
            query_rules[query] = {"required": [item["name"] for item in query_params if item.get("required")],
                                  "properties": {item["name"]: scalar(item["schema"]) for item in query_params}}
            headers = [item for item in params if item["in"] == "header"]
            if any(item["name"] != "Idempotency-Key" or not item.get("required") for item in headers):
                raise ValueError("Unexpected Location header")
            if bool(headers) != (method.upper() != "GET") or set(operation["responses"]) != {"200", *reasons}:
                raise ValueError("Location status/idempotency drift")
            projected[name] = {"method": method.upper(), "path": path, "query": query,
                               "parameters": {item["name"]: scalar(item["schema"]) for item in params if item["in"] == "path"},
                               "idempotency": bool(headers), "errors": reasons, "success": [200]}
    if set(projected) != set(OPERATIONS) or set(projected) & set(operations) or set(query_rules) & set(queries):
        raise ValueError("Duplicate or incomplete Location projection")
    if PATH in inputs and inputs[PATH] != SHA256:
        raise ValueError("Conflicting Location input pin")
    operations.update(projected)
    queries.update(query_rules)
    inputs[PATH] = SHA256
    for reason in reason_values:
        errors.setdefault(reason, None)
