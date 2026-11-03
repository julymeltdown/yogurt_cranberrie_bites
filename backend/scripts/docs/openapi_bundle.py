"""Resolve the generated local OpenAPI index without network or arbitrary file I/O.

Only top-level path/component aliases to the fixed sibling core are supported.
All normal internal component references remain internal after bundling.
"""

from __future__ import annotations

from copy import deepcopy
import json
from pathlib import Path
from typing import Any

CORE_FILENAME = "openapi-core.json"


def pointer_token(value: str) -> str:
    return value.replace("~", "~0").replace("/", "~1")


def core_reference(*tokens: str) -> str:
    return "./" + CORE_FILENAME + "#/" + "/".join(map(pointer_token, tokens))


def split_document(core: dict[str, Any], document: dict[str, Any]) -> dict[str, Any]:
    index = deepcopy(document)
    for path, item in list(index["paths"].items()):
        if core["paths"].get(path) == item:
            index["paths"][path] = {"$ref": core_reference("paths", path)}
    for category, entries in index["components"].items():
        for name, item in list(entries.items()):
            if core["components"].get(category, {}).get(name) == item:
                entries[name] = {"$ref": core_reference("components", category, name)}
    if bundle_document(index, core) != document:
        raise ValueError("OpenAPI split/bundle round trip changed the contract")
    return index


def _resolve(value: Any, core: dict[str, Any], tokens: tuple[str, ...]) -> Any:
    if not isinstance(value, dict) or "$ref" not in value:
        return value
    expected = core_reference(*tokens)
    if value != {"$ref": expected}:
        raise ValueError(f"unsupported OpenAPI alias at {'/'.join(tokens)}")
    target: Any = core
    for token in tokens:
        if not isinstance(target, dict) or token not in target:
            raise ValueError(f"missing OpenAPI target: {expected}")
        target = target[token]
    if not isinstance(target, dict) or "$ref" in target:
        raise ValueError(f"recursive or invalid OpenAPI target: {expected}")
    return deepcopy(target)


def _validate_internal_references(value: Any, root: dict[str, Any]) -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if key == "$ref":
                if not isinstance(child, str) or not child.startswith("#/"):
                    raise ValueError("unresolved external OpenAPI reference")
                target: Any = root
                for token in child[2:].split("/"):
                    token = token.replace("~1", "/").replace("~0", "~")
                    if not isinstance(target, dict) or token not in target:
                        raise ValueError(f"missing internal OpenAPI target: {child}")
                    target = target[token]
            else:
                _validate_internal_references(child, root)
    elif isinstance(value, list):
        for child in value:
            _validate_internal_references(child, root)


def bundle_document(index: dict[str, Any], core: dict[str, Any]) -> dict[str, Any]:
    document = deepcopy(index)
    for path, value in list(document["paths"].items()):
        document["paths"][path] = _resolve(value, core, ("paths", path))
    for category, entries in document["components"].items():
        for name, value in list(entries.items()):
            entries[name] = _resolve(value, core, ("components", category, name))
    _validate_internal_references(document, document)
    return document


def load_document(path: Path) -> dict[str, Any]:
    index = json.loads(path.read_text(encoding="utf-8"))
    core = json.loads(path.with_name(CORE_FILENAME).read_text(encoding="utf-8"))
    return bundle_document(index, core)
