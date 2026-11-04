#!/usr/bin/env python3
"""Real owner/Profile/Participant native join; actual source-bound release, no HTTP claim.

Reuses the retained marker/inode cluster, PID-owned service/port controller and
private PKI helper. Runtime setup follows SELF owner's actual-process component
controller; no synthetic owner, Profile reply or Concert decision is supplied.
"""
from __future__ import annotations
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import subprocess
import uuid

import invalidation_clients_smoke as pki_source
import migration_smoke as migration
import participant_coordinator_sql_smoke as coordinator
import runtime_smoke as runtime

ROOT = Path(__file__).resolve().parents[3]
NATIVE_TEST = "participant::live::joined_tests::native_actual_owner_profile_participant_save_reload_and_abort"
DATABASES = {role: "mcr_participant_live_" + role for role in ("auth", "friend", "profile")}


def require(value, message):
    coordinator.require(value, message)


def snapshot(metadata):
    """Check immutable Git object bytes, allowing an owner's checkout to advance."""
    value = json.loads(metadata.read_text())
    pin = value.get("compiled_source_commit", value["source_commit"])
    sources = value["source_sha256"]
    require(re.fullmatch(r"[0-9a-f]{40}", pin), "canonical qualified source revision required")
    require(sources and all("\n" not in path and ":" not in path and not Path(path).is_absolute()
                            and ".." not in Path(path).parts for path in sources), "source snapshot paths invalid")
    specifications = "".join(pin + ":" + path + "\n" for path in sources)
    result = subprocess.run(["git", "cat-file", "--batch"], input=specifications.encode(),
                            cwd=ROOT, capture_output=True, check=True, timeout=20)
    offset = 0
    for expected in sources.values():
        end = result.stdout.index(b"\n", offset)
        header = result.stdout[offset:end].split()
        require(len(header) == 3 and header[1] == b"blob", "qualified source object missing")
        size = int(header[2])
        offset = end + 1
        content = result.stdout[offset:offset + size]
        require(hashlib.sha256(content).hexdigest() == expected, "qualified source bytes differ")
        offset += size + 1
    require(offset == len(result.stdout), "source snapshot trailing objects")
    return value, pin


def copied_binary(record):
    path = Path(record["path"])
    require(path.is_absolute() and path == path.resolve() and path.is_relative_to("/private/tmp")
            and not path.is_symlink() and path.is_file(), "private frozen binary required")
    require(runtime.digest(path) == record["sha256"], "private frozen binary hash differs")
    migration.executable(path)
    return path


def keys(cluster, count):
    # Node built-in crypto, ephemeral dedicated seeds; no package or key fallback.
    node = shutil.which("node")
    require(node, "Node Ed25519 support required")
    seeds = [base64.b64encode(secrets.token_bytes(32)).decode() for _ in range(count)]
    script = """const c=require('node:crypto');let s='';process.stdin.on('data',x=>s+=x);process.stdin.on('end',()=>process.stdout.write(JSON.stringify(JSON.parse(s).map(v=>c.createPublicKey(c.createPrivateKey({key:Buffer.concat([Buffer.from('302e020100300506032b657004220420','hex'),Buffer.from(v,'base64')]),format:'der',type:'pkcs8'})).export({format:'der',type:'spki'}).subarray(-32).toString('base64')))));"""
    result = subprocess.run([node, "-e", script], input=json.dumps(seeds), text=True,
                            capture_output=True, check=True, env=cluster.env, timeout=5)
    return list(zip(seeds, json.loads(result.stdout)))


def pki(cluster):
    pki_source.pki(cluster)
    directory = cluster.root / "pki"
    (directory / "profile.ext").write_text("basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth,clientAuth\nsubjectAltName=DNS:profile.svc.internal,URI:spiffe://internal/profile\n")
    for args in [
        ["req", "-new", "-newkey", "rsa:2048", "-nodes", "-keyout", "profile.key", "-out", "profile.csr", "-subj", "/CN=profile.svc.internal"],
        ["x509", "-req", "-in", "profile.csr", "-CA", "ca.crt", "-CAkey", "ca.key", "-CAcreateserial", "-out", "profile.crt", "-days", "2", "-extfile", "profile.ext"],
    ]:
        migration.execute(["/usr/bin/openssl", *args], env=cluster.env, cwd=directory, phase="joined-private-pki")
    (directory / "profile.key").chmod(0o600)


def exercise(cluster, ports, children, binaries, binary, report):
    cluster.init()
    with (cluster.data / "postgresql.conf").open("a") as stream:
        stream.write("\nmax_prepared_transactions=64\n")
    cluster.start()
    require(cluster.sql("postgres", "SHOW server_version_num;") == "160014", "exact PostgreSQL16.14 required")
    pki(cluster)
    gateway, relationship, self_profile, decision = keys(cluster, 4)
    credentials = {role.upper() + "_SERVICE_CREDENTIAL": secrets.token_hex(32)
                   for role in ("gateway", "auth", "friend", "profile", "concert", "post", "notification")}
    redactions = (cluster.password, cluster.token, *credentials.values(), gateway[0], relationship[0], self_profile[0], decision[0])
    for role in binaries:
        for kind in ("http", "metrics", "grpc"):
            ports.reserve(role + ":" + kind)
    ports.reserve("concert:grpc")
    ports.reserve("unused")
    urls = {role: "https://" + ports.address(role + ":grpc") for role in [*binaries, "concert"]}
    missing = "https://" + ports.address("unused")
    common = {**cluster.env, **credentials, "ACTOR_DELEGATION_VERIFYING_KEYS": gateway[1],
              "CONCERT_RELATIONSHIP_VERIFYING_KEYS": relationship[1], "CONCERT_OWNER_AUTHORITY_ENABLED": "true",
              "AUTHORITY_CONCERT_SETTLEMENT_ENDPOINT": urls["concert"],
              "AUTHORITY_PROFILE_SETTLEMENT_ENDPOINT": urls["profile"],
              "RUST_LOG": "warn", "TOKIO_WORKER_THREADS": "2"}
    envs = {}
    for role, service in binaries.items():
        database = DATABASES[role]
        cluster.sql("postgres", f"CREATE DATABASE {database} TEMPLATE template0;")
        env = {**common, role.upper() + "_DATABASE_URL": cluster.url(database),
               role.upper() + "_RUNTIME_MODE": "parity", role.upper() + "_DATABASE_MIGRATION_MODE": "external",
               role.upper() + "_SEED_ENABLED": "false"}
        redactions += (cluster.url(database),)
        for kind in ("http", "metrics", "grpc"):
            env[role.upper() + "_" + kind.upper() + "_ADDR"] = ports.address(role + ":" + kind)
        for suffix, filename in [("CERT", role + ".crt"), ("KEY", role + ".key"), ("CA", "ca.crt")]:
            env[role.upper() + "_GRPC_TLS_" + suffix + "_PATH"] = str(cluster.root / "pki" / filename)
        for peer in ("auth", "friend", "profile", "post", "notification"):
            env[role.upper() + "_" + peer.upper() + "_GRPC_ENDPOINT"] = urls.get(peer, missing)
        for flag in ("--migrate", "--verify-migrations"):
            result = migration.execute([str(service), flag], env=env, cwd=cluster.root, phase=role + flag, timeout=30)
            require(all(secret not in result.stdout + result.stderr for secret in redactions), "migration output leaked private configuration")
            report["commands"][role + flag] = result.returncode
        envs[role] = env
    cluster.sql("postgres", f"CREATE DATABASE {coordinator.DATABASE} TEMPLATE template0;")
    report["concert_migration_sha384"] = migration.migration_manifest(ROOT / "backend", "concert-service")
    for path in sorted((ROOT / "backend/services/concert-service/migrations").glob("*.sql")):
        if not path.name.endswith(".down.sql"):
            cluster.sql(coordinator.DATABASE, path.read_text())
    actor, family, event = (str(uuid.uuid4()) for _ in range(3))
    cluster.sql(DATABASES["auth"], f"INSERT INTO users(id,primary_email,password_hash,status) VALUES('{actor}','{actor}@example.invalid','fixture-password','ACTIVE'); INSERT INTO refresh_tokens(jti,token_hash,user_id,family_id,expires_at) VALUES('{uuid.uuid4()}','fixture-refresh','{actor}','{family}',clock_timestamp()+interval '1 hour');")
    cluster.sql(DATABASES["profile"], f"INSERT INTO profiles(user_id,username,nickname,avatar_url,bio) VALUES('{actor}','native-{actor}','Actual native SELF','retained-avatar','Native bio');")
    report["source_bound_release"] = {
        "bundle_sha256": runtime.digest(ROOT / "backend/releases/operator-bundle/quality-v6/bundle.json"),
        "constructor": "ProductReleaseBundle::from_source_bound_files; AvatarPersistence::from_product",
        "operator_admission": "NOT_RUN"}
    envs["auth"].update(AUTH_PUBLIC_APPLICATION_ROUTES_ENABLED="false", AUTH_PASSWORD_RESET_MAILER_MODE="logging",
                        AUTH_PASSWORD_RESET_BASE_URL="https://example.invalid/reset", AUTH_LIFECYCLE_CLEANUP_WORKER_ENABLED="false",
                        AUTH_USER_DELETION_WORKER_ENABLED="false")
    envs["profile"].update(CONCERT_PROFILE_SELF_VERIFYING_KEYS=self_profile[1],
                           PROFILE_PARTICIPANT_DECISION_SIGNING_SEED=decision[0],
                           PROFILE_SELF_AUTH_GRPC_ENDPOINT=urls["auth"], PROFILE_SELF_FRIEND_GRPC_ENDPOINT=urls["friend"],
                           PROFILE_SELF_CONCERT_GRPC_ENDPOINT=urls["concert"])
    for role in ("profile", "auth", "friend"):
        child = runtime.Child(cluster, role, [str(binaries[role])], envs[role], ports,
                              [role + ":" + kind for kind in ("http", "metrics", "grpc")], redactions)
        children.append(child)
        child.wait_listeners()
    env = {**common, "ACTOR_DELEGATION_SIGNING_SEED": gateway[0],
           "CONCERT_RELATIONSHIP_SIGNING_SEED": relationship[0], "CONCERT_PROFILE_SELF_SIGNING_SEED": self_profile[0],
           "PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS": decision[1], "MCR_PARTICIPANT_DECISION_TLS_MODE": "mtls",
           "MCR_PARTICIPANT_COORDINATOR_ROOT": str(cluster.root), "MCR_PARTICIPANT_COORDINATOR_TOKEN": cluster.token,
           "MCR_PARTICIPANT_COORDINATOR_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
           "MCR_PARTICIPANT_COORDINATOR_URL": cluster.url(coordinator.DATABASE),
           "MCR_PARTICIPANT_LIVE_PROFILE_URL": cluster.url(DATABASES["profile"]),
           "MCR_PARTICIPANT_LIVE_ACTOR": actor, "MCR_PARTICIPANT_LIVE_EVENT": event, "MCR_PARTICIPANT_LIVE_FAMILY": family,
           "MCR_PARTICIPANT_LIVE_REPO_ROOT": str(ROOT),
           "MCR_PARTICIPANT_LIVE_CONCERT_ADDR": ports.address("concert:grpc")}
    redactions += (env["MCR_PARTICIPANT_COORDINATOR_URL"],)
    for role in ("auth", "friend", "profile"):
        env["MCR_PARTICIPANT_LIVE_" + role.upper() + "_ENDPOINT"] = urls[role]
    for role in ("concert", "gateway"):
        for suffix, filename in [("CERT", role + ".crt"), ("KEY", role + ".key"), ("CA", "ca.crt")]:
            env[role.upper() + "_GRPC_TLS_" + suffix + "_PATH"] = str(cluster.root / "pki" / filename)
    # Release only our own exclusive reservation immediately before launching
    # the exact copied test process that binds it; no attach or default port.
    ports.release("concert:grpc")
    result = subprocess.run([str(binary), "--ignored", "--exact", NATIVE_TEST, "--test-threads=1", "--nocapture"],
                            env=env, cwd=ROOT, capture_output=True, timeout=55)
    output = result.stdout + result.stderr
    require(all(secret.encode() not in output for secret in redactions), "joined test output leaked private configuration")
    report["commands"][NATIVE_TEST] = result.returncode
    report["native_output"] = output.decode(errors="replace")
    require(result.returncode == 0 and b"1 passed; 0 failed; 0 ignored" in result.stdout, "exact actual native join failed")
    for child in children:
        require(child.listeners(), "actual owner process ended unexpectedly")
    require(cluster.sql(coordinator.DATABASE, "SELECT count(*) FROM concert_participant_effect_witnesses;") == "2", "exact committed avatar/preferences witnesses required")
    require(cluster.sql(coordinator.DATABASE, "SELECT count(*) FROM concert_participant_profile_terminal_receipts WHERE outcome='COMMITTED';") == "1", "actual Profile commit receipt required")
    require(cluster.sql(coordinator.DATABASE, "SELECT count(*) FROM concert_participant_profile_terminal_receipts WHERE outcome='ABORTED';") == "1", "actual Profile abort receipt required")
    require(cluster.sql(coordinator.DATABASE, "SELECT count(*) FROM concert_product_owner_scopes WHERE delivery_completed_at IS NOT NULL;") == "6", "all six completed RPC envelopes must settle actual owner scopes")
    report.update(state="PASS", actual_save_reload="PASS", actual_profile_prepare_commit="PASS",
                  actual_expired_prepare_abort="PASS", actual_owner_delivery_completion="PASS",
                  gateway_http="NOT_RUN", operator_admission="NOT_RUN",
                  aborted_lost_envelope_scope="quarantined; no fabricated serialization/completion")


def capture_diagnostics(cluster, children):
    """Read-only evidence before owned cleanup; no content, keys or capabilities."""
    queries = {
        "root_scopes": (coordinator.DATABASE, "SELECT coalesce(json_agg(x),'[]') FROM (SELECT phase,binding->>'http_method' AS method,serialized_at IS NOT NULL AS serialized,delivery_completed_at IS NOT NULL AS completed FROM concert_product_owner_scopes) x"),
        "effect_witnesses": (coordinator.DATABASE, "SELECT coalesce(json_agg(x),'[]') FROM (SELECT mutation_kind,expected_revision,committed_revision,concert_full_xid,pg_xact_status(concert_full_xid::xid8) AS native_status FROM concert_participant_effect_witnesses) x"),
        "concert_profile_operations": (coordinator.DATABASE, "SELECT coalesce(json_agg(x),'[]') FROM (SELECT decision,expected_revision,committed_revision,concert_full_xid,pg_xact_status(concert_full_xid::xid8) AS native_status FROM concert_participant_profile_operations) x"),
        "profile_operations": (DATABASES["profile"], "SELECT coalesce(json_agg(x),'[]') FROM (SELECT full_xid,pg_xact_status(full_xid::xid8) AS native_status FROM concert_self_profile_operations) x"),
        "profile_observations": (DATABASES["profile"], "SELECT coalesce(json_agg(x),'[]') FROM (SELECT observed_status FROM concert_self_profile_settled_observations) x"),
    }
    for role in ("auth", "friend"):
        queries[role + "_leases"] = (DATABASES[role], "SELECT coalesce(json_agg(x),'[]') FROM (SELECT state,scope_settlement IS NOT NULL AS scope_settled FROM concert_owner_guard_leases) x")
        queries[role + "_participants"] = (DATABASES[role], "SELECT coalesce(json_agg(x),'[]') FROM (SELECT producer,state FROM concert_owner_guard_participants) x")
    diagnostics = {}
    for name, (database, query) in queries.items():
        try:
            diagnostics[name] = json.loads(cluster.sql(database, query))
        except Exception as error:
            # An early migration/startup failure must retain the primary error.
            diagnostics[name] = {"unavailable": type(error).__name__}
    diagnostics["redacted_peer_logs"] = {}
    for child in children:
        try:
            diagnostics["redacted_peer_logs"][child.name] = child.diagnostic()
        except Exception as error:
            diagnostics["redacted_peer_logs"][child.name] = type(error).__name__
    return diagnostics


def run(args):
    report = {"state": "FAIL", "commands": {}, "scope": "actual native owner/Profile/Participant join and strict source-bound release constructor; initial rows are fixtures; no public HTTP/operator acceptance"}
    cluster, ports, children = None, runtime.Ports(), []
    previous = {}
    def interrupt(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.signal(sig, interrupt)
        pin = coordinator.validate_binary(args.binary, args.receipt, ROOT)
        owners, owner_pin = snapshot(args.owner_artifacts)
        own_profile, profile_pin = snapshot(args.profile_artifacts)
        binaries = {role: copied_binary(owners["extra_artifacts"][role + "-server"]) for role in ("auth", "friend")}
        binaries["profile"] = copied_binary(own_profile["artifacts"]["profile-bin"])
        report.update(source_commit=pin["source_commit"], binary_sha256=pin["sha256"],
                      owner_source_commit=owner_pin, profile_source_commit=profile_pin,
                      peer_binary_sha256={role: runtime.digest(path) for role, path in binaries.items()},
                      driver_sha256=runtime.digest(Path(__file__)))
        require(shutil.disk_usage(ROOT).free >= 8 * 1024**3, "eight GiB free required")
        cluster = migration.Cluster({name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS})
        try:
            exercise(cluster, ports, children, binaries, args.binary, report)
        finally:
            report["diagnostics"] = capture_diagnostics(cluster, children)
    except (Exception, KeyboardInterrupt) as error:
        report["failure"] = error.detail if isinstance(error, migration.SmokeError) else type(error).__name__
    finally:
        try:
            if cluster is not None:
                stopped = []
                for child in reversed(children):
                    stopped.append(child.stop())
                report["process_cleanup"] = stopped
                require(all(not result.get("forced") and not result.get("unexpected_exit") and result["exit_code"] == 0 for result in stopped), "actual service shutdown failed")
                cluster.cleanup()
                require(not cluster.root.exists(), "owned cluster remains")
            ports.close()
            report["cleanup"] = "PASS"
        except BaseException as error:
            report.update(state="FAIL", cleanup="FAIL", cleanup_failure=type(error).__name__)
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("binary", "receipt", "owner-artifacts", "profile-artifacts", "pg-bin", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    require(all(path.is_absolute() for path in vars(args).values()), "absolute inputs required")
    fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    report = run(args)
    with os.fdopen(fd, "w") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    print(json.dumps({key: report.get(key) for key in ("state", "failure", "cleanup", "actual_save_reload")}))
    return 0 if report["state"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
