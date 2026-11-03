#!/usr/bin/env python3
"""Generate/check both local OpenAPI artifacts or emit the bundled public contract."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

import openapi_core
from openapi import auth, platform_docs, post_notification, profile_social
from openapi.verification import extend_document
from openapi_bundle import CORE_FILENAME, bundle_document, split_document

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "docs/api/openapi.json"
CORE_OUTPUT = OUTPUT.with_name(CORE_FILENAME)
# Keep the existing assembler's validation helpers available to tooling.
collect_registrations = openapi_core.collect_registrations
validate_operation_ids = openapi_core.validate_operation_ids
validate_document = openapi_core.validate_document


def serialize(document: dict[str, Any]) -> str:
    return json.dumps(document, indent=2, ensure_ascii=False) + "\n"


def build_document() -> dict[str, Any]:
    document = extend_document(openapi_core.build_document())
    validate_document(document)
    return document


def build_artifacts() -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    core = openapi_core.build_document()
    document = extend_document(core)
    validate_document(document)
    index = split_document(core, document)
    return core, index, document


def build_content() -> str:
    """Return the checked-in index; build_document returns the bundled contract."""
    _, index, _ = build_artifacts()
    return serialize(index)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="check both generated artifacts")
    mode.add_argument("--bundle", action="store_true", help="write standalone JSON to stdout")
    args = parser.parse_args(argv)
    core, index, document = build_artifacts()
    if args.bundle:
        print(serialize(document), end="")
        return 0
    artifacts = ((CORE_OUTPUT, serialize(core)), (OUTPUT, serialize(index)))
    if args.check:
        for path, expected in artifacts:
            if not path.exists() or path.read_text(encoding="utf-8") != expected:
                raise SystemExit(f"{path.relative_to(ROOT)} is stale; run scripts/docs/generate-openapi.py")
        if bundle_document(index, core) != document:
            raise SystemExit("OpenAPI bundle does not match generated contract")
        print(f"OpenAPI generation check passed: {sum(map(len, document['paths'].values()))} operations; 2 artifacts")
        return 0
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    for path, content in artifacts:
        path.write_text(content, encoding="utf-8")
    print(f"generated 2 OpenAPI artifacts with {sum(map(len, document['paths'].values()))} operations")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
