"""Complete staged source manifest and receiving-owner build-checkpoint validation.
No builder, approval token, or historical authoring-SHA gate exists here.
The receiving owner records source_manifest(repo) in the actual clean build receipt.
"""
from __future__ import annotations
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess

TARGET = "process_child_recovery"
SUPPORT = "backend/services/concert-service/tests/process_child_recovery_support"
REQUIRED = {f"backend/services/concert-service/tests/{TARGET}.rs"}
REQUIRED.update(f"{SUPPORT}/{name}.rs" for name in
                ("environment", "capture", "owners", "release", "fixture", "fault", "unfinished", "missing_key", "fairness", "profile_read_fault", "uncertain_producer", "uncertain"))
REQUIRED.update(f"{SUPPORT}/{name}.py" for name in ("controller", "checkpoint", "qualify", "controller_cr04", "qualify_cr04", "controller_cr06", "qualify_cr06", "controller_uncertain", "qualify_uncertain"))
SOURCE_SUFFIXES = {".rs", ".sql", ".toml", ".proto", ".py", ".json"}
EXCLUDED_DIRS = {"target", "targets", "node_modules", ".git", "__pycache__"}

def git(repo: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], check=True,
                          text=True, capture_output=True).stdout

def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

def source_manifest(repo: Path) -> dict[str, str]:
    """Superset of owner_authority_support/qualify.py's source_manifest.

    Includes new staged test/support/scripts, all backend tracked inputs, source
    suffixes even when ignored, contracts, selected runtime release/art resources
    and toolchain configs. Never traverses build/target directories.
    """
    tracked = set(git(repo, "ls-files", "--", "backend", "contracts", "frontend/public",
                      "frontend/src/entities/avatar/character-source-pack.json",
                      ".cargo", "rust-toolchain.toml", "rust-toolchain", "Cargo.toml", "Cargo.lock")
                  .splitlines())
    if not REQUIRED.issubset(tracked):
        raise ValueError("new process test/support/controller must be staged and committed before checkpointing")
    paths = {p for p in tracked if not set(PurePosixPath(p).parts) & EXCLUDED_DIRS
             and (repo / p).is_file()}
    for prefix in ("backend", "contracts"):
        for parent, directories, files in os.walk(repo / prefix, followlinks=False):
            directories[:] = [d for d in directories if d not in EXCLUDED_DIRS
                               and not (Path(parent) / d).is_symlink()]
            for name in files:
                path = Path(parent) / name
                if path.suffix in SOURCE_SUFFIXES:
                    paths.add(path.relative_to(repo).as_posix())
    result = {}
    for relative in sorted(paths):
        path = repo / relative
        if path.is_symlink():
            raise ValueError("source inputs must not escape through symlinks")
        result[relative] = sha(path)
    return result

def verify_checkpoint(repo: Path, checkpoint_path: Path) -> tuple[Path, str, str]:
    """Consume the actual receiving-owner build receipt, never authoring provenance.

    Accept the existing receipt spelling source_sha256 or the explicit
    source_manifest spelling. If both are present they must agree exactly.
    """
    checkpoint = json.loads(checkpoint_path.read_text())
    if checkpoint.get("target_name") != TARGET:
        raise ValueError("exact process integration target checkpoint required")
    if git(repo, "status", "--porcelain", "--untracked-files=all").strip():
        raise ValueError("complete clean committed build checkout required")
    commit = git(repo, "rev-parse", "HEAD").strip()
    if checkpoint.get("source_commit") != commit:
        raise ValueError("build checkpoint source commit drift")
    recorded = checkpoint.get("source_manifest", checkpoint.get("source_sha256"))
    if not isinstance(recorded, dict) or not recorded:
        raise ValueError("complete build source_manifest required")
    if ("source_manifest" in checkpoint and "source_sha256" in checkpoint
            and checkpoint["source_manifest"] != checkpoint["source_sha256"]):
        raise ValueError("ambiguous build source manifests")
    for relative in recorded:
        parts = PurePosixPath(relative)
        if parts.is_absolute() or ".." in parts.parts or set(parts.parts) & EXCLUDED_DIRS:
            raise ValueError("unsafe source manifest path")
    if recorded != source_manifest(repo):
        raise ValueError("complete build source/runtime resource manifest drift")
    # An outside private copy is usable only if it matches these freshly staged
    # scripts. Do not silently run a stale draft controller against another build.
    for name in ("controller.py", "checkpoint.py"):
        if sha(Path(__file__).with_name(name)) != recorded[f"{SUPPORT}/{name}"]:
            raise ValueError("executing controller differs from staged build checkpoint")
    if checkpoint.get("exit_code") != 0:
        raise ValueError("successful actual Cargo checkpoint required")
    command = checkpoint.get("command", [])
    if (not isinstance(command, list) or "--no-run" not in command
            or "--locked" not in command or "--offline" not in command
            or command.count("--test") != 1
            or command[command.index("--test") + 1] != TARGET):
        raise ValueError("exact single-target locked offline no-run receipt required")
    binary = Path(checkpoint["binary"])
    if not binary.is_absolute():
        raise ValueError("absolute prebuilt executable required")
    binary = binary.resolve(strict=True)
    if not stat.S_ISREG(binary.stat().st_mode) or not os.access(binary, os.X_OK):
        raise ValueError("prebuilt integration executable required")
    # The builder records its canonical target path while it owns the lease.
    # Copied-process qualification runs after release and must not stat or
    # resolve the now-unowned shared target directory.
    target = Path(checkpoint["target"])
    if not target.is_absolute() or ".." in target.parts or os.path.normpath(str(target)) != str(target):
        raise ValueError("canonical absolute build target path required")
    if binary == target or target in binary.parents:
        raise ValueError("outside-target frozen executable required")
    digest = checkpoint.get("binary_sha256")
    if not isinstance(digest, str) or len(digest) != 64 or sha(binary) != digest:
        raise ValueError("build checkpoint executable SHA256 mismatch")
    return binary, digest, commit


def profile_certificate(cluster, core) -> None:
    """Issue the real Profile workload identity in the exact owned fixture PKI."""
    cluster.assert_owned()
    directory = cluster.root / "pki"
    extension = directory / "profile.ext"
    extension.write_text("basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:profile.svc.internal,URI:spiffe://internal/profile\n")
    openssl = core.executable(Path("/usr/bin/openssl"))
    for args in [
        ["req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", "profile.key", "-out", "profile.csr", "-subj", "/CN=profile.svc.internal"],
        ["x509", "-req", "-in", "profile.csr", "-CA", "ca.crt", "-CAkey", "ca.key", "-CAcreateserial", "-out", "profile.crt", "-days", "2", "-extfile", str(extension)],
    ]:
        core.execute([str(openssl), *args], env=cluster.env, cwd=directory, phase="fixture-profile-pki", timeout=30)
    (directory / "profile.key").chmod(0o600)
