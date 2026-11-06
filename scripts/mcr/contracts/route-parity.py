#!/usr/bin/env python3
"""Fail closed when a registered Concert command has no exact axum template.

The command registry and the frozen C04 operation contract name public command
surfaces. Without an exact axum template the Gateway falls into its generic
fallback and never reaches the Concert producer, so this gap is a build error.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REGISTRY = Path("backend/crates/concert-application/src/commands/registry.rs")
OPERATIONS = Path("contracts/domain/v1/operations.json")
GATEWAY_HTTP = Path("backend/services/api-gateway/src/adapters/in/http")

OPERATION_RE = re.compile(
    r'id:\s*"([^"]+)",\s*method:\s*"([^"]+)",\s*path:\s*"([^"]+)"', re.S
)
ROUTE_RE = re.compile(r'\.route\(\s*"([^"]+)"\s*,\s*')
METHOD_RE = re.compile(r"\b(get|post|put|patch|delete|head|options)\s*\(")
PATH_PARAM_RE = re.compile(r"\{([^}]+)\}")


def normalize_contract_path(path: str) -> str:
    # `{eventId}` in the contract becomes `:event_id` in axum templates.
    def replacement(match: re.Match[str]) -> str:
        name = match.group(1)
        snake = re.sub(r"(?<!^)([A-Z])", r"_\1", name).lower()
        return f":{snake}"

    return PATH_PARAM_RE.sub(replacement, path)


def parse_registry(root: Path) -> list[tuple[str, str, str]]:
    source = (root / REGISTRY).read_text(encoding="utf-8")
    rows = OPERATION_RE.findall(source)
    if not rows:
        raise SystemExit(f"no RegisteredOperation rows in {REGISTRY}")
    return [(op, method.upper(), path) for op, method, path in rows]


def parse_contract(root: Path) -> list[tuple[str, str, str]]:
    rows = json.loads((root / OPERATIONS).read_text(encoding="utf-8"))
    if not isinstance(rows, list) or not rows:
        raise SystemExit(f"no operations in {OPERATIONS}")
    commands = []
    for row in rows:
        method = str(row["method"]).upper()
        if method == "GET" or str(row.get("write_policy", "")).upper() == "READ":
            continue
        commands.append((row["operationId"], method, row["path"]))
    return commands


def parse_axum_templates(root: Path) -> set[tuple[str, str]]:
    templates: set[tuple[str, str]] = set()
    http = root / GATEWAY_HTTP
    if not http.is_dir():
        raise SystemExit(f"missing gateway HTTP tree: {GATEWAY_HTTP}")
    for source in sorted(http.rglob("*.rs")):
        text = source.read_text(encoding="utf-8")
        for match in ROUTE_RE.finditer(text):
            path = match.group(1)
            index = match.end()
            depth = 0
            while index < len(text):
                char = text[index]
                if char == "(":
                    depth += 1
                elif char == ")":
                    if depth == 0:
                        break
                    depth -= 1
                index += 1
            expression = text[match.end() : index]
            for method in METHOD_RE.findall(expression):
                templates.add((method.upper(), path))
    return templates


def collect_commands(root: Path) -> dict[tuple[str, str], str]:
    commands: dict[tuple[str, str], str] = {}
    for operation, method, path in parse_registry(root) + parse_contract(root):
        commands[(method, normalize_contract_path(path))] = operation
    return commands


def missing_templates(root: Path) -> list[tuple[str, str, str]]:
    templates = parse_axum_templates(root)
    commands = collect_commands(root)
    return [
        (operation, method, path)
        for (method, path), operation in sorted(
            commands.items(), key=lambda item: (item[0][1], item[0][0])
        )
        if (method, path) not in templates
    ]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Concert command/axum route parity")
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[3],
        help="repository root (defaults to this checkout)",
    )
    args = parser.parse_args(argv)
    root = args.root.resolve()

    registered = parse_registry(root)
    contracted = parse_contract(root)
    templates = parse_axum_templates(root)
    commands = collect_commands(root)
    missing = missing_templates(root)

    print(
        f"route parity: registry={len(registered)} contract_commands={len(contracted)} "
        f"templates={len(templates)} commands={len(commands)} missing={len(missing)}"
    )
    for operation, method, path in missing:
        print(f"MISSING {method:6} {path}  ({operation})")
    if missing:
        print("route parity failed: every registered command needs an exact axum template")
        return 1
    print("route parity ok: every registered command has an exact axum template")
    return 0


if __name__ == "__main__":
    sys.exit(main())
