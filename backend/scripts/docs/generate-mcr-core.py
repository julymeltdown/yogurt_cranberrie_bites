#!/usr/bin/env python3
"""Assemble the MCR target contract from the FINAL inherited auth generator.

This does not register runtime routes. Producer implementation is qualified
separately; x-mcr-runtime-status makes that boundary explicit per operation.
"""
from __future__ import annotations

import argparse
import copy
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUTPUT = ROOT / "backend/docs/api/mcr-core.openapi.json"
PUBLIC_OUTPUT = ROOT / "contracts/generated/mcr-core.openapi.json"
AUTH_IDS = {
    "signupWithEmail", "verifyEmail", "resendVerificationEmail", "login",
    "refreshAccessToken", "logout", "requestPasswordReset", "confirmPasswordReset",
    "authorizeOAuth", "completeOAuthCallback", "receiveAppleOAuthFormPost",
    "completeAppleOAuthFormPost", "linkOAuthIdentity", "deleteAccount",
    "getAccountDeletion", "getAccountDeletionStatus",
}


def inherited_document():
    spec = importlib.util.spec_from_file_location(
        "mcr_inherited_final", Path(__file__).with_name("generate-openapi.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.build_document()


def json_schema(value):
    """Translate inherited OpenAPI 3.0 nullable/exclusive bounds to 2020-12."""
    if isinstance(value, list):
        return [json_schema(item) for item in value]
    if not isinstance(value, dict):
        return value
    result = {key: json_schema(item) for key, item in value.items() if key != "nullable"}
    for bound in ("minimum", "maximum"):
        exclusive = "exclusive" + bound.title()
        if isinstance(result.get(exclusive), bool):
            enabled = result.pop(exclusive)
            if enabled and bound in result:
                result[exclusive] = result.pop(bound)
    if value.get("nullable"):
        return {"anyOf": [result, {"type": "null"}]}
    return result


def load_schemas(root=ROOT):
    files = sorted((root / "contracts/assets/v1").glob("*.schema.json"))
    files += sorted((root / "contracts/core/v1").glob("*.schema.json"))
    if len(files) < 6:
        raise ValueError("Core and reviewed asset contract inputs are required")
    documents = [json.loads(file.read_text()) for file in files]
    refs = {}
    for document in documents:
        for name in document["$defs"]:
            refs[f'{document["$id"]}#/$defs/{name}'] = f"#/components/schemas/{name}"

    def rewrite(value, base):
        if isinstance(value, list):
            return [rewrite(item, base) for item in value]
        if not isinstance(value, dict):
            return value
        result = {key: rewrite(item, base) for key, item in value.items()}
        if "$ref" in result:
            original = result["$ref"]
            absolute = base + original if original.startswith("#") else original
            if absolute not in refs:
                raise ValueError(f"Unregistered schema reference: {absolute}")
            result["$ref"] = refs[absolute]
        return result

    schemas = {}
    for document in documents:
        for name, schema in document["$defs"].items():
            if name in schemas:
                raise ValueError(f"Duplicate schema name: {name}")
            schemas[name] = rewrite(schema, document["$id"])
    return schemas


def build_document():
    inherited = inherited_document()
    document = {
        "openapi": "3.1.0",
        "info": {"title": "MCR core target contract", "version": "0.1.0",
                 "description": "C03 v0.1 frozen core target; runtime implementation and optional OAuth handoff are separately qualified."},
        "servers": [{"url": "/"}],
        "paths": {},
        "components": json_schema(copy.deepcopy(inherited["components"])),
        "x-mcr-contract-state": "CONTRACT_READY",
    }
    found = set()
    for path, methods in inherited["paths"].items():
        for method, operation in methods.items():
            if operation.get("operationId") not in AUTH_IDS:
                continue
            if not path.startswith("/auth/"):
                raise ValueError(f"Unexpected inherited auth path: {path}")
            target = json_schema(copy.deepcopy(operation))
            target["x-mcr-upstream"] = {"method": method.upper(), "path": path,
                                        "operationId": target["operationId"]}
            target["x-mcr-runtime-status"] = "ADAPTATION_REQUIRED"
            target["x-mcr-retry"] = "NO_AUTOMATIC_REPLAY"
            target["x-mcr-cache"] = "NO_STORE"
            target["x-mcr-browser-exposure"] = (
                "DISABLED_UNTIL_HANDOFF_QUALIFIED" if "oauth" in path else "TARGET")
            if method in {"post", "delete"} and not ("oauth" in path and "callback" in path):
                target["x-mcr-origin-policy"] = "EXACT_ALLOWLIST_AND_X_MCR_INTENT_BROWSER"
                target["x-mcr-origin-rejection"] = {
                    "status": 403, "schema": {"$ref": "#/components/schemas/OriginErrorResponse"},
                    "downstream_dispatch": False,
                }
                target["responses"]["403"] = {"description": "Origin/intent denied, or forbidden by Auth",
                    "content": {"application/json": {"schema": {"$ref": "#/components/schemas/AuthForbiddenResponse"}}}}
                target.setdefault("parameters", []).append({
                    "name": "X-MCR-Intent", "in": "header", "required": True,
                    "schema": {"type": "string", "const": "browser"},
                    "description": "Require this header AND an exactly allowed Origin; no wildcard/null Origin."})
            if target["operationId"] == "logout":
                target["security"] = []
                target["description"] = target.get("description", "") + " MCR permits missing refresh cookie for local logout; Origin/intent remain required."
            document["paths"].setdefault("/api" + path, {})[method] = target
            found.add(target["operationId"])
    if found != AUTH_IDS:
        raise ValueError(f"Inherited auth operations changed: {AUTH_IDS ^ found}")
    schemas = document["components"]["schemas"]
    # Source echoes bounded opaque correlation strings, not UUID-only values.
    schemas["ErrorResponse"]["properties"]["traceId"] = {
        "type": ["string", "null"], "maxLength": 128}
    for name in ("EmailSignupRequest", "PasswordResetConfirmRequest"):
        key = "newPassword" if name == "PasswordResetConfirmRequest" else "password"
        schemas[name]["properties"][key].pop("minLength", None)
        schemas[name]["properties"][key]["x-mcr-utf8-bytes"] = {"minimum": 8, "maximum": 128}
    core = load_schemas()
    for name in core:
        if name in schemas:
            raise ValueError(f"Core/inherited schema collision: {name}")
    schemas.update(core)
    register = json.loads((ROOT / "contracts/core/v1/operations.json").read_text())
    for operation in register:
        path, method, identity = operation["path"], operation["method"], operation["operationId"]
        if identity in found:
            raise ValueError(f"Duplicate operationId: {identity}")
        found.add(identity)
        parameters = []
        for parameter in ("eventId", "userId", "placeId"):
            if "{" + parameter + "}" in path:
                schema = {"type": "string", "format": "uuid"} if parameter != "placeId" else {"$ref": "#/components/schemas/PlaceId"}
                parameters.append({"name": parameter, "in": "path", "required": True, "schema": schema})
        error_reasons = {
            400: ["INVALID_INPUT"], 401: ["AUTHENTICATION_REQUIRED"],
            403: ["PERMISSION_DENIED", "CONSENT_REQUIRED"], 404: ["RESOURCE_UNAVAILABLE"],
            409: ["VERSION_CONFLICT", "MAP_VERSION_MISMATCH", "EVENT_READ_ONLY"],
            413: ["PAYLOAD_TOO_LARGE"], 415: ["INVALID_MEDIA_TYPE"], 422: ["INVALID_INPUT"],
            429: ["RATE_LIMITED"], 500: ["INTERNAL_ERROR"], 502: ["DEPENDENCY_UNAVAILABLE"],
            503: ["DEPENDENCY_UNAVAILABLE"], 504: ["DEADLINE_EXCEEDED"],
        }
        responses = {str(code): {"description": description, "content": {"application/json": {
            "schema": {"allOf": [{"$ref": "#/components/schemas/CoreErrorResponse"},
                                   {"properties": {"details": {"properties": {
                                       "reason": {"enum": error_reasons[code]}}}}}]}}}}
            for code, description in [(400, "Invalid input"), (401, "Authentication required"),
                                      (403, "Not permitted"), (404, "Resource unavailable"),
                                      (409, "Revision or state conflict"), (413, "Payload too large"),
                                      (415, "JSON media type required"), (422, "Invalid field types"),
                                      (429, "Rate limited"), (500, "Internal failure"),
                                      (502, "Bad dependency response"), (503, "Required dependency unavailable"),
                                      (504, "Dependency deadline exceeded")]}
        responses["200"] = {"description": "Success", "content": {"application/json": {
            "schema": {"$ref": f'#/components/schemas/{operation["response"]}'}}}}
        target = {"operationId": identity, "parameters": parameters, "responses": responses,
                  "security": [{"BearerAuth": []}] if operation["authenticated"] else [],
                  "x-mcr-runtime-status": "NOT_IMPLEMENTED", "x-mcr-cache": "NO_STORE",
                  "x-mcr-retry": ("READ_ONCE_AFTER_REFRESH" if operation["authenticated"]
                                  else "PUBLIC_READ_NO_AUTH_REFRESH") if method == "get" else "NO_AUTOMATIC_REPLAY"}
        if operation.get("request"):
            target["requestBody"] = {"required": True, "content": {"application/json": {
                "schema": {"$ref": f'#/components/schemas/{operation["request"]}'}}}}
        if method in {"put", "patch", "post", "delete"}:
            target["x-mcr-origin-policy"] = "EXACT_ALLOWLIST_AND_X_MCR_INTENT_BROWSER"
            target["x-mcr-origin-rejection"] = {
                "status": 403, "schema": {"$ref": "#/components/schemas/OriginErrorResponse"},
                "downstream_dispatch": False,
            }
            target["responses"]["403"]["content"]["application/json"]["schema"] = {
                "anyOf": [target["responses"]["403"]["content"]["application/json"]["schema"],
                          {"$ref": "#/components/schemas/OriginErrorResponse"}]}
            target["parameters"].append({"name": "X-MCR-Intent", "in": "header", "required": True,
                                         "schema": {"type": "string", "const": "browser"}})
        document["paths"].setdefault(path, {})[method] = target
    document["paths"] = dict(sorted(document["paths"].items()))
    # Exclude unrelated inherited content DTOs from the browser core surface.
    def referenced_schemas(value):
        if isinstance(value, list):
            return set().union(*(referenced_schemas(item) for item in value))
        if not isinstance(value, dict):
            return set()
        names = set()
        reference = value.get("$ref", "")
        if reference.startswith("#/components/schemas/"):
            names.add(reference.split("/")[3])
        for child in value.values():
            names.update(referenced_schemas(child))
        return names
    pending = set(core) | referenced_schemas(document["paths"])
    pending.update(referenced_schemas({key: value for key, value in document["components"].items()
                                      if key != "schemas"}))
    retained = set()
    while pending:
        name = pending.pop()
        if name in retained:
            continue
        if name not in schemas:
            raise ValueError(f"Missing component schema: {name}")
        retained.add(name)
        pending.update(referenced_schemas(schemas[name]) - retained)
    document["components"]["schemas"] = {name: schemas[name] for name in sorted(retained)}
    return document


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    content = json.dumps(build_document(), ensure_ascii=False, indent=2) + "\n"
    for output in (OUTPUT, PUBLIC_OUTPUT):
        if args.check:
            if not output.exists() or output.read_text() != content:
                parser.exit(1, f"MCR core contract is stale: {output.relative_to(ROOT)}; run generate-mcr-core.py\n")
        else:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(content)
    print("MCR core target contract " + ("checked" if args.check else "generated"))


if __name__ == "__main__":
    main()
