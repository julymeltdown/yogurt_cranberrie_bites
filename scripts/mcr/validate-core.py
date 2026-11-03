#!/usr/bin/env python3
"""Offline validation of C03 target examples, never runtime acceptance."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys

from jsonschema import Draft202012Validator, FormatChecker, ValidationError, validators

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "backend/scripts/docs"
sys.path.insert(0, str(DOCS))


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


generator = load_module("mcr_core_generator", DOCS / "generate-mcr-core.py")
assets = load_module("mcr_asset_validator", Path(__file__).with_name("validate-assets.py"))


def byte_length(_validator, bounds, instance, _schema):
    if isinstance(instance, str):
        try:
            length = len(instance.encode("utf-8"))
        except UnicodeEncodeError:
            yield ValidationError("password must be valid Unicode")
            return
        if not bounds["minimum"] <= length <= bounds["maximum"]:
            yield ValidationError("password must contain 8–128 UTF-8 bytes")


CoreValidator = validators.extend(Draft202012Validator, {"x-mcr-utf8-bytes": byte_length})


def require(condition, message):
    if not condition:
        raise ValueError(message)


def validate(value, schema_name, document, *, actor_id=None, event_id=None,
             target_user_id=None, place_id=None, operation_id=None, http_status=None):
    require(schema_name in document["components"]["schemas"], "unknown core schema")
    wrapper = {"$schema": "https://json-schema.org/draft/2020-12/schema",
               "$ref": f"#/components/schemas/{schema_name}",
               "components": document["components"]}
    CoreValidator(wrapper, format_checker=FormatChecker()).validate(value)
    if operation_id is not None:
        operations = [operation for methods in document["paths"].values()
                      for operation in methods.values() if operation["operationId"] == operation_id]
        require(len(operations) == 1 and http_status is not None, "unknown operation or missing response status")
        response = operations[0]["responses"].get(str(http_status))
        require(response is not None, "undocumented operation response status")
        wrapper.pop("$ref")
        wrapper.update(response["content"]["application/json"]["schema"])
        CoreValidator(wrapper, format_checker=FormatChecker()).validate(value)
    if schema_name == "PublicProfile" and target_user_id is not None:
        require(value["user_id"] == target_user_id, "public profile belongs to a different target user")
    if schema_name in {"PlaceResponse", "PlaceList"} and event_id is not None:
        require(value["event_id"] == event_id, "place response belongs to a different event")
    if schema_name == "PlaceResponse" and place_id is not None:
        require(value["place"]["place_id"] == place_id, "place response belongs to a different place")
    if schema_name == "AuthCapabilities":
        require({row["provider"] for row in value["providers"]} == {"EMAIL", "GOOGLE", "APPLE", "X"},
                "provider capability IDs must be unique and complete")
    if schema_name == "PrivateParticipant":
        require(value["user_id"] == value["profile"]["user_id"], "private/profile identity mismatch")
        if actor_id is not None:
            require(value["user_id"] == actor_id, "private response belongs to a different actor")
        if event_id is not None:
            require(value["event_id"] == event_id, "private response belongs to a different event")
    if schema_name == "Bootstrap":
        validate(value["participant"], "PrivateParticipant", document,
                 actor_id=actor_id, event_id=event_id or value["event"]["event_id"])
        require(value["event"]["event_id"] == value["participant"]["event_id"], "bootstrap event mismatch")
        require(value["avatar"] == value["participant"]["profile"]["avatar"], "bootstrap avatar projection mismatch")
        if value["event"]["phase"] in {"READ_ONLY", "ARCHIVED"}:
            require(not value["capabilities"]["mutations"]["available"], "read-only event permits mutations")
        bundle = {"avatar": value["avatar_manifest"], "map": value["map_manifest"],
                  "config": value["avatar"]["config"]}
        bundle.update({f"theme-{index}": theme for index, theme in enumerate(value["theme_manifests"])})
        assets.validate_bundle(bundle)
    if schema_name == "PlaceList":
        ids = [place["place_id"] for place in value["items"]]
        require(len(ids) == len(set(ids)), "duplicate place ID")


def validate_fixtures(document=None):
    document = document or generator.build_document()
    manifest = assets.read_json(ROOT / "contracts/fixtures/core/index.json")
    require(bool(manifest), "empty core fixture index")
    covered = set()
    for example in manifest:
        relative = Path(example["file"])
        require(len(relative.parts) == 1 and relative.suffix == ".json", "unsafe fixture path")
        require(example["file"] not in covered, "duplicate core fixture")
        covered.add(example["file"])
        value = assets.read_json(ROOT / "contracts/fixtures/core" / relative)
        validate(value, example["schema"], document,
                 actor_id=example.get("actor_id"), event_id=example.get("event_id"),
                 target_user_id=example.get("target_user_id"), place_id=example.get("place_id"),
                 operation_id=example.get("operation_id"), http_status=example.get("http_status"))
    actual = {p.name for p in (ROOT / "contracts/fixtures/core").glob("*.json")} - {"index.json"}
    require(covered == actual, "unindexed or missing core fixture")
    return len(covered)


def main():
    try:
        count = validate_fixtures()
    except (ValueError, ValidationError, OSError) as error:
        print(f"Core fixture validation failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps({"state": "DRAFT_VALIDATED", "fixtures": count,
                      "producer_runtime": "NOT_RUN", "contract_freeze": "PENDING"}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
