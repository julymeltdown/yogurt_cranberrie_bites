#!/usr/bin/env python3
"""Actual Auth/Friend/Profile read/Track/abort proof; no root/HTTP acceptance."""
import argparse
import base64
import json
import os
from pathlib import Path
import secrets
import shutil
import signal
import subprocess
import time
import uuid

import concert_self_profile_native as native
import invalidation_clients_smoke as pki_source
import migration_smoke as migration
import runtime_smoke as runtime
from concert_core_smoke import command, digest, require


def owner_binaries(metadata, source_root):
    value = json.loads(metadata.read_text())
    require(source_root.is_absolute() and source_root.resolve() == source_root, "canonical-owner-source-root-required")
    require(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source_root, text=True).strip() == value["source_commit"], "owner-source-revision-mismatch")
    for name, expected in value["source_sha256"].items():
        path = source_root / name
        require(path.resolve().is_relative_to(source_root) and digest(path) == expected, "owner-source-bytes-mismatch")
    result = {}
    for owner in ("auth", "friend"):
        record = value["extra_artifacts"][owner + "-server"]
        path = migration.executable(Path(record["path"]))
        require(not path.is_symlink() and digest(path) == record["sha256"], "frozen-owner-binary-mismatch")
        result[owner] = path
    return result


def key(cluster, label):
    seed = secrets.token_bytes(32)
    private = cluster.root / (label + ".der")
    public = cluster.root / (label + ".pub")
    runtime.private_file(private, bytes.fromhex("302e020100300506032b657004220420") + seed)
    migration.execute(["/usr/bin/openssl", "pkey", "-inform", "DER", "-in", str(private), "-pubout", "-outform", "DER", "-out", str(public)], env=cluster.env, cwd=cluster.root, phase="fixture-key")
    value = public.read_bytes()
    require(len(value) == 44 and value[:12] == bytes.fromhex("302a300506032b6570032100"), "key-shape-refused")
    private.unlink()
    public.unlink()
    return base64.b64encode(seed).decode(), base64.b64encode(value[12:]).decode()


def pki(cluster):
    pki_source.pki(cluster)
    directory = cluster.root / "pki"
    role = "profile"
    (directory / (role + ".ext")).write_text(f"basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:{role}.svc.internal,URI:spiffe://internal/{role}\n")
    for args in [["req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", role + ".key", "-out", role + ".csr", "-subj", "/CN=profile.svc.internal"], ["x509", "-req", "-in", role + ".csr", "-CA", "ca.crt", "-CAkey", "ca.key", "-CAcreateserial", "-out", role + ".crt", "-days", "2", "-extfile", role + ".ext"]]:
        migration.execute(["/usr/bin/openssl", *args], env=cluster.env, cwd=directory, phase="profile-pki")
    (directory / "profile.key").chmod(0o600)


def exercise(cluster, ports, children, own, owners, report):
    cluster.init()
    with (cluster.data / "postgresql.conf").open("a") as stream:
        stream.write("\nmax_prepared_transactions=64\n")
    cluster.start()
    require(cluster.sql("postgres", "show server_version_num;") == "160014", "postgres-16.14-required")
    pki(cluster)
    gateway_seed, gateway_key = key(cluster, "gateway")
    relation_seed, relation_key = key(cluster, "relation")
    self_seed, self_key = key(cluster, "self")
    decision_seed, _ = key(cluster, "decision")
    credentials = {role.upper() + "_SERVICE_CREDENTIAL": secrets.token_hex(32) for role in ("gateway", "auth", "friend", "profile", "concert", "post", "notification")}
    envs = {}
    binaries = {**owners, "profile": own["profile-bin"]}
    for role in binaries:
        for kind in ("http", "metrics", "grpc"):
            ports.reserve(role + ":" + kind)
    # Deliberately unavailable private root/decision/unused peers. These reserved
    # endpoints cannot certify completion. This independent read probe must keep
    # actual root owner guards quarantined; decision/HTTP acceptance is NOT_RUN.
    ports.reserve("unavailable-root")
    missing = "https://" + ports.address("unavailable-root")
    urls = {role: "https://" + ports.address(role + ":grpc") for role in binaries}
    common = {**cluster.env, **credentials, "ACTOR_DELEGATION_VERIFYING_KEYS": gateway_key,
              "CONCERT_RELATIONSHIP_VERIFYING_KEYS": relation_key, "CONCERT_OWNER_AUTHORITY_ENABLED": "true",
              "AUTHORITY_CONCERT_SETTLEMENT_ENDPOINT": missing, "AUTHORITY_PROFILE_SETTLEMENT_ENDPOINT": urls["profile"],
              "RUST_LOG": "warn", "TOKIO_WORKER_THREADS": "2"}
    redactions = tuple([cluster.password, cluster.token, gateway_seed, relation_seed, self_seed, decision_seed, *credentials.values()])
    for role, binary in binaries.items():
        database = "mcr_profile_self" if role == "profile" else "mcr_self_owner_" + role
        cluster.sql("postgres", f"create database {database} template template0;")
        env = {**common, role.upper() + "_RUNTIME_MODE": "parity", role.upper() + "_DATABASE_URL": cluster.url(database),
               role.upper() + "_DATABASE_MIGRATION_MODE": "external", role.upper() + "_SEED_ENABLED": "false"}
        redactions += (cluster.url(database),)
        for kind in ("http", "metrics", "grpc"):
            env[role.upper() + "_" + kind.upper() + "_ADDR"] = ports.address(role + ":" + kind)
        for suffix, name in [("CERT", role + ".crt"), ("KEY", role + ".key"), ("CA", "ca.crt")]:
            env[role.upper() + "_GRPC_TLS_" + suffix + "_PATH"] = str(cluster.root / "pki" / name)
        for peer in ("auth", "friend", "profile", "post", "notification"):
            env[role.upper() + "_" + peer.upper() + "_GRPC_ENDPOINT"] = urls.get(peer, missing)
        command(report, role + "-migrate", [binary, "--migrate"], env, cluster.root, secrets=redactions)
        command(report, role + "-verify", [binary, "--verify-migrations"], env, cluster.root, secrets=redactions)
        envs[role] = env
    actor, family, event = map(str, (uuid.uuid4(), uuid.uuid4(), uuid.uuid4()))
    cluster.sql("mcr_self_owner_auth", f"insert into users(id,primary_email,password_hash,status) values('{actor}','{actor}@example.invalid','fixture-password','ACTIVE');insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values('{uuid.uuid4()}','fixture-refresh','{actor}','{family}',clock_timestamp()+interval '1 hour');")
    cluster.sql(native.DB, f"insert into profiles(user_id,username,nickname,avatar_url,bio) values('{actor}','native-{actor}','Actual native SELF','retained-avatar','Native bio');")
    envs["auth"].update(AUTH_PUBLIC_APPLICATION_ROUTES_ENABLED="false", AUTH_PASSWORD_RESET_MAILER_MODE="logging", AUTH_PASSWORD_RESET_BASE_URL="https://example.invalid/reset", AUTH_LIFECYCLE_CLEANUP_WORKER_ENABLED="false", AUTH_USER_DELETION_WORKER_ENABLED="false")
    envs["profile"].update(CONCERT_PROFILE_SELF_VERIFYING_KEYS=self_key, PROFILE_PARTICIPANT_DECISION_SIGNING_SEED=decision_seed, PROFILE_SELF_AUTH_GRPC_ENDPOINT=urls["auth"], PROFILE_SELF_FRIEND_GRPC_ENDPOINT=urls["friend"], PROFILE_SELF_CONCERT_GRPC_ENDPOINT=missing)
    for role in ("profile", "auth", "friend"):
        child = runtime.Child(cluster, role, [str(binaries[role])], envs[role], ports, [role + ":" + kind for kind in ("http", "metrics", "grpc")], redactions)
        children.append(child)
        child.wait_listeners()
    env = {**cluster.env, **credentials, "ACTOR_DELEGATION_VERIFYING_KEYS": gateway_key,
           "CONCERT_RELATIONSHIP_SIGNING_SEED": relation_seed, "CONCERT_PROFILE_SELF_SIGNING_SEED": self_seed,
           "MCR_SELF_PROFILE_ROOT": str(cluster.root), "MCR_SELF_PROFILE_TOKEN": cluster.token,
           "MCR_SELF_PROFILE_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}", "MCR_SELF_PROFILE_URL": cluster.url(native.DB),
           "MCR_SELF_PROFILE_ACTOR": actor, "MCR_SELF_PROFILE_FAMILY": family, "MCR_SELF_PROFILE_EVENT": event}
    env.update({"MCR_SELF_PROFILE_" + role.upper() + "_ENDPOINT": url for role, url in urls.items()})
    name = "concert_self_profile::native_rpc::actual_owner_read_track_and_native_abort_settlement"
    result = command(report, "actual-owners-read-rpc", [own["test"], "--ignored", "--exact", name, "--test-threads=1"], env, cluster.root, secrets=redactions)
    require(b"test result: ok. 1 passed; 0 failed;" in result.stdout, "exact-real-owner-test-required")
    for child in children:
        require(child.listeners(), "owned-service-must-remain-alive")
    require(cluster.sql(native.DB, "select count(*) from concert_self_profile_read_references;") == "1", "exact-read-reference-required")
    require(cluster.sql(native.DB, "select count(*) from concert_self_profile_receipts;") == "0", "read-must-not-create-write-receipt")
    time.sleep(6)  # Original request expires; no completion proof may be invented.
    for role in ("auth", "friend"):
        count = cluster.sql("mcr_self_owner_" + role, "select count(*) from pg_prepared_xacts where database=current_database();")
        require(count == "1", "missing-root-must-retain-owner-guard:" + role)
    report.update(actual_read_track_native_abort="PASS", missing_root_quarantine="PASS", root_scope_release="NOT_RUN", result="PASS")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cargo-json", "frozen-artifacts", "owner-artifacts", "owner-source-root", "pg-bin", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    require(all(path.is_absolute() for path in vars(args).values()), "absolute-inputs-required")
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = {"result": "FAIL", "commands": {}, "scope": "real owner/Profile read component only", "actual_concert_decision": "NOT_RUN", "gateway_http": "NOT_RUN"}
    cluster, ports, children = None, runtime.Ports(), []
    previous = {}
    def interrupt(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupt)
        require(shutil.disk_usage(native.ROOT).free >= 8 * 1024**3, "eight-gibibyte-floor-required")
        own = native.artifacts(args.cargo_json, args.frozen_artifacts)
        require(bool(json.loads(args.frozen_artifacts.read_text()).get("source_sha256")), "source-bound-rpc-artifacts-required")
        owners = owner_binaries(args.owner_artifacts, args.owner_source_root)
        report.update(owner_metadata_sha256=digest(args.owner_artifacts), artifact_sha256={k:digest(v) for k,v in {**owners, **own}.items()}, source_revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=native.ROOT, text=True).strip(), controller_sha256=digest(Path(__file__)))
        cluster = migration.Cluster({name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS})
        report["owned_directory"] = str(cluster.root)
        exercise(cluster, ports, children, own, owners, report)
    except (Exception, KeyboardInterrupt) as error:
        report["failure"] = str(error) if type(error).__name__ == "CheckFailure" else type(error).__name__
    finally:
        try:
            if cluster is not None:
                results, failures = [], []
                for child in reversed(children):
                    try:
                        results.append(child.stop())
                    except BaseException as error:
                        failures.append(type(error).__name__)
                report["process_cleanup"] = results
                require(not failures and all(child.process.poll() is not None for child in children), "owned-process-cleanup-failed")
                # Test-only destruction of our isolated fixture. This never
                # purports to prove production root completion or guard release.
                cluster.cleanup()
                require(not cluster.root.exists(), "owned-root-remains")
                require(all(not row.get("forced") and not row.get("unexpected_exit") and row.get("exit_code") == 0 for row in results), "service-shutdown-not-clean")
            ports.close()
            report["cleanup"] = "PASS"
        except BaseException:
            report.update(result="FAIL", cleanup="FAIL")
        for sig, handler in previous.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({key: report.get(key) for key in ("result", "failure", "actual_read_track_native_abort", "missing_root_quarantine", "cleanup")}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
