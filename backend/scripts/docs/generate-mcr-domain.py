#!/usr/bin/env python3
"""Extend the retained final/core pipeline with C04 and C05 HTTP candidates.

The frozen core artifact is not rewritten. This wider review artifact is not a
runtime route inventory or an independently authored Auth contract.
"""
from __future__ import annotations

import argparse
import copy
import importlib.util
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[3]
OUTPUT = ROOT / "contracts/generated/mcr-domain.openapi.json"
METHODS = {"get", "post", "put", "patch", "delete", "options", "head", "trace"}


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def core_document():
    directory = Path(__file__).resolve().parent
    # The retained final assembler imports sibling openapi modules.
    sys.path.insert(0, str(directory))
    try:
        spec = importlib.util.spec_from_file_location("mcr_domain_core", directory / "generate-mcr-core.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module.build_document()
    finally:
        sys.path.pop(0)


def local_ref(name):
    return {"$ref": "#/components/schemas/" + name}


def resolve(document, reference):
    if not reference.startswith("#/"):
        raise ValueError(f"External reference in bundle: {reference}")
    value = document
    try:
        for token in reference[2:].split("/"):
            value = value[token.replace("~1", "/").replace("~0", "~")]
    except (KeyError, TypeError) as error:
        raise ValueError(f"Unresolved bundle reference: {reference}") from error
    return value


def rewrite(value, references, base=""):
    if isinstance(value, list):
        return [rewrite(item, references, base) for item in value]
    if not isinstance(value, dict):
        return value
    result = {key: rewrite(child, references, base) for key, child in value.items()}
    if "$ref" in result:
        original = result["$ref"]
        absolute = base + original if original.startswith("#") else original
        if absolute not in references:
            raise ValueError(f"Unregistered source reference: {absolute}")
        result["$ref"] = references[absolute]
    return result


def add_schemas(document, root):
    """Preserve core names; namespace new definitions and rewrite every ref."""
    references, inputs = {}, []
    for family in ("assets", "core", "domain", "realtime"):
        paths = sorted((root / "contracts" / family / "v1").glob("*.schema.json"))
        if not paths:
            raise ValueError(f"Missing schema family: {family}")
        for path in paths:
            source = read(path)
            group = path.name.removesuffix(".schema.json")
            for name, schema in source["$defs"].items():
                target = name if family in {"assets", "core"} else (
                    f"Domain_{group}_{name}" if family == "domain" else f"Realtime_{group}_{name}")
                absolute = f'{source["$id"]}#/$defs/{name}'
                if absolute in references:
                    raise ValueError(f"Duplicate source schema ID: {absolute}")
                references[absolute] = local_ref(target)["$ref"]
                if family in {"domain", "realtime"}:
                    inputs.append((target, schema, source["$id"]))
                elif target not in document["components"]["schemas"]:
                    raise ValueError(f"Missing core component: {target}")
    schemas = document["components"]["schemas"]
    for target, schema, base in inputs:
        if target in schemas:
            raise ValueError(f"Component collision: {target}")
        schemas[target] = rewrite(schema, references, base)
    document["components"]["schemas"] = dict(sorted(schemas.items()))
    return references


def domain_ref(name):
    group, definition = name.split(".", 1)
    return local_ref(f"Domain_{group}_{definition}")


def response(schema, description="Success"):
    return {"description": description, "headers": {
        "Cache-Control": {"required": True, "schema": {"type": "string", "const": "no-store"}}},
        "content": {"application/json": {"schema": schema}}}


def constrained_error(schema, reasons):
    return {"allOf": [schema, {"properties": {"details": {"properties": {
        "reason": {"enum": reasons}}}}}]}


def intent(target):
    target["x-mcr-origin-policy"] = "EXACT_ALLOWLIST_AND_X_MCR_INTENT_BROWSER"
    target["parameters"].append({"name": "X-MCR-Intent", "in": "header", "required": True,
                                 "schema": {"type": "string", "const": "browser"}})


def query_parameters(document, schema):
    query = resolve(document, schema["$ref"])
    if query.get("type") != "object" or query.get("additionalProperties") is not False:
        raise ValueError("Closed query object required")
    return [{"name": name, "in": "query", "required": name in query.get("required", []),
             "style": "form", "explode": False, "allowReserved": False,
             "schema": copy.deepcopy(value)} for name, value in query["properties"].items()]


def domain_operation(row, document, references, media_types):
    method = row["method"]
    if row["rule"] == "moderate_user":
        if (row.get("authority_scope") != "PATH_EVENT_ONLY"
                or row["path"] != "/api/concert/events/{eventId}/admin/users/{userId}/moderation"
                or row["actor"] != "MODERATOR"):
            raise ValueError("Concert user moderation requires exact event-local authority scope")
    if row["request_location"] != ("query" if method in {"get", "delete"} else "body"):
        raise ValueError(f'Invalid request location: {row["operationId"]}')
    query = domain_ref(row["query_schema"])
    target = {"operationId": row["operationId"],
              "parameters": [{"name": name, "in": "path", "required": True,
                              "schema": rewrite(schema, references)}
                             for name, schema in row["path_parameters"].items()],
              "security": [] if row["actor"] == "PUBLIC" else [{"BearerAuth": []}],
              "responses": {code: response(constrained_error(domain_ref("common.DomainError"), reasons),
                                            "Domain error") for code, reasons in row["errors"].items()},
              "x-mcr-query-schema": query, "x-mcr-closed-query": True,
              "x-mcr-runtime-status": row["implementation"], "x-mcr-retry": row["retry"],
              "x-mcr-cache": "NO_STORE", "x-mcr-actor": row["actor"],
              "x-mcr-rule": row["rule"], "x-mcr-write-policy": row["write_policy"]}
    if row["rule"] == "moderate_user":
        target["x-mcr-authority-scope"] = row["authority_scope"]
    if "command_modes" in row:
        target["x-mcr-command-modes"] = copy.deepcopy(row["command_modes"])
    target["parameters"].extend(query_parameters(document, query))
    if row["response"] == "BINARY":
        success = response({}, "Sanitized authorized media bytes")
        success["content"] = {mime: {"schema": {"type": "string", "format": "binary"}}
                              for mime in media_types}
        success["headers"]["X-Content-Type-Options"] = {
            "required": True, "schema": {"type": "string", "const": "nosniff"}}
    else:
        success = response(domain_ref(row["response"]))
    target["responses"][str(row["success_status"])] = success
    if row["pending_response"]:
        target["responses"][str(row["pending_status"])] = response(
            domain_ref(row["pending_response"]), "Pending; not successful completion or media readiness")
    if method not in {"get", "delete"} and row["request"]:
        content = ({mime: {"schema": {"type": "string", "format": "binary"}} for mime in media_types}
                   if row["request"] == "BINARY" else
                   {"application/json": {"schema": domain_ref(row["request"])}})
        target["requestBody"] = {"required": True, "content": content}
    if method != "get":
        intent(target)
    if row["idempotency_key"]:
        target["parameters"].append({"name": "Idempotency-Key", "in": "header", "required": True,
                                     "schema": domain_ref("common.Uuid")})
    return target


def realtime_operation(row, references):
    target = {"operationId": row["operationId"],
              "parameters": [{"name": "eventId", "in": "path", "required": True,
                              "schema": domain_ref("common.Uuid")}],
              "security": [{"BearerAuth": []}] if row["authenticated"] else [],
              "responses": {code: response(constrained_error(
                  rewrite({"$ref": row["error_schema"]}, references), reasons), "World HTTP error")
                  for code, reasons in row["errors"].items()},
              "x-mcr-runtime-status": row["runtime_status"], "x-mcr-retry": row["retry"],
              "x-mcr-cache": row["cache"]}
    target["responses"][str(row["response_status"])] = response(local_ref("Realtime_world_" + row["response"]))
    if row["request"]:
        target["requestBody"] = {"required": True, "content": {"application/json": {
            "schema": local_ref("Realtime_world_" + row["request"])}}}
    if row.get("origin_policy"):
        intent(target)
        schema = target["responses"]["403"]["content"]["application/json"]["schema"]
        target["responses"]["403"]["content"]["application/json"]["schema"] = {
            "anyOf": [schema, rewrite({"$ref": row["origin_error_schema"]}, references)]}
    return target


def validate_document(document):
    """Local reference/registration integrity, not a complete OpenAPI validator."""
    identities, routes = set(), {}
    for path, methods in document["paths"].items():
        normalized = re.sub(r"\{[^{}]+\}", "{}", path)
        if normalized in routes and routes[normalized] != path:
            raise ValueError(f"Equivalent path templates: {path}, {routes[normalized]}")
        routes[normalized] = path
        for method, operation in methods.items():
            if method not in METHODS:
                continue
            identity = operation["operationId"]
            if identity in identities:
                raise ValueError(f"Duplicate operationId: {identity}")
            identities.add(identity)
            parameters = [resolve(document, p["$ref"]) if "$ref" in p else p
                          for p in operation.get("parameters", [])]
            keys = [(p["in"], p["name"]) for p in parameters]
            if len(keys) != len(set(keys)):
                raise ValueError(f"Duplicate parameter: {identity}")
            expected = set(re.findall(r"\{([^{}]+)\}", path))
            actual = {p["name"] for p in parameters if p["in"] == "path" and p.get("required") is True}
            if expected != actual:
                raise ValueError(f"Path parameter mismatch: {identity}")
            if method in {"get", "delete"} and "requestBody" in operation:
                raise ValueError(f"Unexpected read/delete body: {identity}")

    def walk(value):
        if isinstance(value, list):
            for child in value:
                walk(child)
        elif isinstance(value, dict):
            if "$ref" in value:
                resolve(document, value["$ref"])
            for child in value.values():
                walk(child)
    walk(document)


def build_document(root=ROOT):
    document = core_document()
    document["info"] = {"title": "MCR domain HTTP review candidate", "version": "0.1.0",
                        "description": "C03 frozen core plus C04 candidate and C05 world HTTP. Not runtime coverage or C04 acceptance."}
    document["x-mcr-contract-state"] = "REVIEW_CANDIDATE"
    references = add_schemas(document, root)
    policy = read(root / "contracts/domain/v1/policy.json")
    document["x-mcr-domain-policy"] = policy
    document["x-mcr-realtime-policy"] = read(root / "contracts/realtime/v1/policy.json")
    media_types = read(root / "contracts/domain/v1/feed.schema.json")["$defs"]["InitUpload"]["properties"]["content_type"]["enum"]
    for family in ("domain", "realtime"):
        for row in read(root / "contracts" / family / "v1/operations.json"):
            target = (domain_operation(row, document, references, media_types) if family == "domain"
                      else realtime_operation(row, references))
            methods = document["paths"].setdefault(row["path"], {})
            if row["method"] in methods:
                raise ValueError(f'Duplicate route: {row["method"]} {row["path"]}')
            methods[row["method"]] = target
    document["paths"] = dict(sorted(document["paths"].items()))
    validate_document(document)
    return document


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    content = json.dumps(build_document(), ensure_ascii=False, indent=2) + "\n"
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text(encoding="utf-8") != content:
            parser.exit(1, "MCR domain candidate is stale; run generate-mcr-domain.py\n")
    else:
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(content, encoding="utf-8")
    print("MCR domain HTTP REVIEW_CANDIDATE " + ("checked" if args.check else "generated"))


if __name__ == "__main__":
    main()
