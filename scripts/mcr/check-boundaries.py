#!/usr/bin/env python3
"""Check an MCR worker's index and working-tree diffs for FE/BE boundary mistakes."""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import os
from pathlib import Path, PurePosixPath
import posixpath
import re
import stat
import subprocess
import sys


DOC_SUFFIXES = {".md", ".rst", ".txt"}
SOURCE_SUFFIXES = {".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".mts", ".cts", ".rs"}
CENTRAL_DIRS = {
    ".git", ".github", ".gitlab", ".circleci", ".buildkite", "ci",
    "contracts", "generated", "proto", "docker", "compose",
    "infra", "infrastructure", "deploy", "deployment", "k8s", "kubernetes",
    "helm", "terraform", "scripts",
}
CENTRAL_NAMES = {
    "agents.md", "cargo.toml", "cargo.lock", "package.json", "package-lock.json",
    "pnpm-lock.yaml", "pnpm-workspace.yaml", "yarn.lock", "bun.lock", "bun.lockb",
    "npm-shrinkwrap.json", ".npmrc", "build.rs", "rust-toolchain", "rust-toolchain.toml",
    "makefile", "justfile", "deno.json", "deno.jsonc", "deno.lock",
    ".gitignore", ".gitattributes", ".gitmodules",
    ".gitlab-ci.yml", "jenkinsfile", "azure-pipelines.yml", "bitbucket-pipelines.yml",
}
# These are known composition locations, not a complete architecture inventory.
COMPOSITION = re.compile(
    r"^(?:frontend/src/(?:main|index|app|router|routes|root)\.[^/]+|"
    r"frontend/src/(?:router|routes)/.*|"
    r"frontend/src/app/(?:router|routes|root|app|index)\.[^/]+|"
    r"frontend/src/app/(?:router|routes)/.*|"
    r"backend/services/[^/]+/src/(?:main|lib|router|routes)\.rs|"
    r"backend/services/[^/]+/src/(?:router|routes)/.*|"
    r"backend/services/[^/]+/src/adapters/in/http/(?:mod|router|routes)\.rs|"
    r"backend/services/[^/]+/src/adapters/in/http/(?:router|routes)/.*)$"
)
# A deliberately small lexer: preserve string tokens and ignore JS/Rust comments.
TOKENS = re.compile(
    r"(?P<comment>//[^\n]*|/\*[\s\S]*?\*/)|"
    r'(?P<raw>r(?P<hashes>\#{0,16})"[\s\S]*?"(?P=hashes))|'
    r'''(?P<string>"(?:\\[\s\S]|[^"\\])*"|'(?:\\.|[^'\\\n])*'|`(?:\\[\s\S]|[^`\\])*`)|'''
    r"(?P<word>[A-Za-z_$][\w$]*)|(?P<punct>[^\s])"
)


@dataclass(frozen=True)
class Change:
    status: str
    paths: tuple[str, ...]


def git(root: Path, *args: str) -> bytes:
    result = subprocess.run(
        ["git", "-C", str(root), *args], capture_output=True,
        env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"},
    )
    if result.returncode:
        raise ValueError(f"git {args[0]} failed: {os.fsdecode(result.stderr).strip()!r}")
    return result.stdout


def safe_path(path: str) -> bool:
    # Git paths are slash-separated; never interpret options, globs, or traversal.
    return bool(path) and not path.startswith("/") and "\\" not in path and all(
        part not in {"", ".", ".."} for part in path.split("/")
    ) and "\x00" not in path


def changed_paths(root: Path, base: str, cached: bool = False) -> list[Change]:
    fields = git(
        root, "diff", "--no-ext-diff", "--no-textconv", "--ignore-submodules=none",
        "--name-status", "-z", "--find-renames", *(["--cached"] if cached else []), base, "--",
    ).split(b"\x00")
    changes = []
    index = 0
    while index < len(fields) - 1:
        status = fields[index].decode("ascii")
        count = 2 if status.startswith(("R", "C")) else 1
        paths = tuple(os.fsdecode(p) for p in fields[index + 1:index + 1 + count])
        if len(paths) != count or not status.startswith(tuple("ACDMRTUXB")):
            raise ValueError("Unexpected Git name-status output")
        changes.append(Change(status, paths))
        index += 1 + count
    if not cached:
        for path in git(root, "ls-files", "--others", "--exclude-standard", "-z").split(b"\x00"):
            if path:
                changes.append(Change("?", (os.fsdecode(path),)))
    return changes


def central_reason(path: str) -> str | None:
    lower = path.casefold()
    parts = PurePosixPath(lower).parts
    name = parts[-1]
    if any(part in CENTRAL_DIRS for part in parts):
        return "integrator-owned contracts/generated output/CI/scripts/infrastructure"
    if lower == "backend/docs/api" or lower.startswith("backend/docs/api/"):
        return "integrator-owned HTTP contract inputs"
    if name in CENTRAL_NAMES or name.endswith((".lock", ".lockb", ".tf", ".tfvars")):
        return "integrator-owned manifest/lock/build configuration"
    if name.startswith((".env", "dockerfile", "docker-compose", "compose.", "tsconfig")) or re.match(
        r"(?:vite|vitest|playwright|eslint|postcss|tailwind|webpack)\.config\.", name
    ):
        return "integrator-owned environment/build configuration"
    if COMPOSITION.fullmatch(lower):
        return "integrator-owned root router/service composition"
    return None


def valid_doc(path: str, lane: str) -> bool:
    return safe_path(path) and not any(c in path for c in "*?[]") and (
        PurePosixPath(path).suffix in DOC_SUFFIXES
        or (path.startswith("docs/mcr/") and PurePosixPath(path).suffix == ".json")
    ) and (
        path.startswith("docs/mcr/") or path.startswith(lane + "/")
    ) and central_reason(path) is None


def path_reason(path: str, lane: str, docs: set[str]) -> str | None:
    if not safe_path(path):
        return "unsafe/non-canonical repository path"
    other = "frontend" if lane == "backend" else "backend"
    if path == other or path.startswith(other + "/"):
        return f"opposite-lane write ({other})"
    if reason := central_reason(path):
        return reason
    if path in docs:
        return None
    if not path.startswith(lane + "/"):
        return f"outside {lane}/ and exact assigned documentation files"
    if PurePosixPath(path).suffix in DOC_SUFFIXES or "/docs/" in path:
        return "documentation requires an exact --allow-doc assignment"
    return None


def regular_file(root: Path, path: str) -> Path:
    current = root
    for part in path.split("/"):
        current = current / part
        mode = current.lstat().st_mode
        if stat.S_ISLNK(mode):
            raise ValueError("symlink paths are not scanned or permitted")
    if not stat.S_ISREG(mode):
        raise ValueError("non-regular file (including submodule/directory) is not permitted")
    return current


def index_blob(root: Path, path: str) -> str:
    entries = git(root, "ls-files", "--stage", "-z", "--", ":(literal)" + path).split(b"\x00")
    if len(entries) != 2:
        raise ValueError("missing or unmerged index entry")
    mode, blob, stage = entries[0].split(b"\t", 1)[0].split()
    if mode not in {b"100644", b"100755"} or stage != b"0":
        raise ValueError("symlink, submodule, or unmerged index entry is not permitted")
    return blob.decode("ascii")


def crosses(path: str, specifier: str, other: str) -> bool:
    specifier = specifier.split("?", 1)[0].split("#", 1)[0]
    if specifier.startswith("."):
        target = posixpath.normpath(posixpath.join(posixpath.dirname(path), specifier))
        return target == other or target.startswith(other + "/")
    return bool(re.search(rf"(?:^|[/@:]){other}(?:[/.:]|$)", specifier))


def cross_imports(path: str, source: str, lane: str) -> list[str]:
    other = "frontend" if lane == "backend" else "backend"
    tokens = [m for m in TOKENS.finditer(source) if m.lastgroup != "comment"]
    findings = []
    for i, token in enumerate(tokens):
        before = [t.group() for t in tokens[max(0, i - 4):i]]
        value = token.group()
        specifier = None
        if token.lastgroup in {"string", "raw"}:
            direct = before and before[-1] in {"from", "import"}
            call = len(before) >= 2 and before[-1] == "(" and before[-2] in {"import", "require"}
            resolve = before == ["require", ".", "resolve", "("]
            macro = len(before) >= 3 and before[-2:] == ["!", "("] and before[-3] in {
                "include", "include_str", "include_bytes", "include_dir",
            }
            attribute = before == ["#", "[", "path", "="] or before == ["#", "[", "folder", "="]
            if direct or call or resolve or macro or attribute:
                quote = value.index('"') if token.lastgroup == "raw" else 0
                specifier = value[quote + 1:value.rfind(value[quote])]
                if value.startswith("`") and "${" in specifier:
                    specifier = None  # Computed imports require manual review.
        elif token.lastgroup == "word" and value == other and (
            before[-1:] == ["use"] or before[-3:] == ["use", ":", ":"]
            or before[-2:] == ["extern", "crate"]
        ):
            specifier = value
        if specifier is not None and crosses(path, specifier, other):
            line = source.count("\n", 0, token.start()) + 1
            findings.append(f"{path!r}:{line}: cross-implementation import/embed {specifier!r}")
    return findings


def check(root: Path, base: str, lane: str, docs: set[str]) -> tuple[list[str], int]:
    violations = []
    paths = set()
    for cached in (True, False):
        snapshot = "index" if cached else "working tree"
        scanned = set()
        for change in changed_paths(root, base, cached):
            paths.update(change.paths)
            if change.status.startswith(("U", "X", "B")):
                violations.append(f"{change.paths!r}: unresolved or unsupported Git change {change.status}")
            for path in change.paths:
                if reason := path_reason(path, lane, docs):
                    violations.append(f"{path!r} ({snapshot}, {change.status}): {reason}")
            path = change.paths[-1]
            if change.status.startswith("D") or path in scanned or not safe_path(path):
                continue
            scanned.add(path)
            try:
                blob = index_blob(root, path) if cached else None
                file = None if cached else regular_file(root, path)
                if PurePosixPath(path).suffix in SOURCE_SUFFIXES:
                    source = (
                        git(root, "cat-file", "blob", blob).decode("utf-8")
                        if cached else file.read_text(encoding="utf-8")
                    )
                    violations.extend(f"{snapshot}: {v}" for v in cross_imports(path, source, lane))
            except (OSError, UnicodeError, ValueError) as error:
                violations.append(f"{path!r} ({snapshot}): cannot safely inspect file: {str(error)!r}")
    return violations, len(paths)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="worker assignment's base commit/ref")
    parser.add_argument("--lane", choices=("backend", "frontend"), required=True)
    parser.add_argument(
        "--allow-doc", action="append", default=[], metavar="PATH",
        help="exact assigned .md/.rst/.txt path, or .json under docs/mcr/; no directories/globs",
    )
    args = parser.parse_args()
    try:
        root = Path(os.fsdecode(git(Path.cwd(), "rev-parse", "--show-toplevel")).removesuffix("\n"))
        base = git(root, "rev-parse", "--verify", "--end-of-options", args.base + "^{commit}").decode().strip()
        for path in args.allow_doc:
            if not valid_doc(path, args.lane):
                raise ValueError(f"Invalid documentation assignment: {path!r}")
        violations, count = check(root, base, args.lane, set(args.allow_doc))
    except (OSError, ValueError) as error:
        print(f"BOUNDARIES ERROR: {str(error)!r}", file=sys.stderr)
        return 2
    if violations:
        print("BOUNDARIES FAIL", file=sys.stderr)
        for violation in violations:
            print(f"- {violation}", file=sys.stderr)
        return 1
    print(f"BOUNDARIES PASS: {args.lane}, {count} changes vs {base}; manual assignment/import review required")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
