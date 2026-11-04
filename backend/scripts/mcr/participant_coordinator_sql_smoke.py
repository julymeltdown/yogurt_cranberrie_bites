#!/usr/bin/env python3
"""Qualify 0027 SQL only in a new owned PG16 cluster; no Rust/live-flow claim.

Uses migration_smoke.Cluster's marker/inode/postmaster/socket checks. The retained
helper's revision and the exact migration manifest are included in the report.
"""
from __future__ import annotations
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import subprocess
import time
import uuid
import migration_smoke as migration

DATABASE = "mcr_participant_coordinator_sql"
NATIVE_TEST = "participant::live::coordinator_tests::native_guarded_coordinator_cas_rollback_and_decision_recovery"
ENDPOINT_TEST = "participant::live::decision_endpoint_tests::native_decision_service_endpoint"
CLIENT_TEST = "participant::live::decision_endpoint_tests::native_decision_service_client"


def require(value, message):
    if not value:
        raise migration.SmokeError("assertion", message)


class Session:
    """One actual producer connection; never substitutes a pool connection XID."""
    def __init__(self, cluster):
        cluster.assert_postmaster()
        self.process = subprocess.Popen(
            [str(cluster.tools["psql"]), "-X", "--no-password", "-qAt", "-v", "ON_ERROR_STOP=1"],
            env=cluster.client_env(DATABASE), cwd=cluster.root,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            bufsize=0)
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ)
        self.buffer = b""

    def call(self, sql):
        marker = "mcr_done_" + uuid.uuid4().hex
        self.process.stdin.write((sql + "\nSELECT '" + marker + "';\n").encode())
        self.process.stdin.flush()
        stop = time.monotonic() + 5
        lines = []
        while time.monotonic() < stop:
            while b"\n" in self.buffer:
                line, self.buffer = self.buffer.split(b"\n", 1)
                text = line.decode()
                if text == marker:
                    return "\n".join(lines).strip()
                lines.append(text)
            if not self.selector.select(max(0, stop - time.monotonic())):
                break
            data = os.read(self.process.stdout.fileno(), 65536)
            if not data:
                raise migration.SmokeError("producer-session", "producer connection ended before marker")
            self.buffer += data
        raise migration.SmokeError("producer-session", "producer query exceeded bound")

    def close(self):
        self.selector.close()
        if self.process.poll() is None:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.terminate()
                self.process.wait(timeout=5)
        self.process.stdout.close()
        self.process.stderr.close()


def register(cluster, session):
    session.call("BEGIN;")
    identity = json.loads(session.call("""SELECT json_build_object(
      'system_identifier',(SELECT system_identifier::text FROM pg_control_system()),
      'database_oid',(SELECT oid::bigint FROM pg_database WHERE datname=current_database()),
      'full_xid',pg_current_xact_id()::text,'pid',pg_backend_pid(),
      'started',(SELECT backend_start FROM pg_stat_activity WHERE pid=pg_backend_pid()));"""))
    # Every interpolation below is generated here or returned by this owned PG.
    # Nothing comes from command-line SQL, a browser, an environment URL, or a peer.
    operation, actor, event, family, epoch = [uuid.uuid4() for _ in range(5)]
    gid = "mcr_sp_" + operation.hex
    cluster.sql(DATABASE, f"""INSERT INTO public.concert_participant_profile_operations(
      operation_id,actor_id,event_id,session_family_id,parent_digest,body_digest,
      guard_binding_digest,owner_binding_proto,release_digest,expected_revision,
      concert_system_identifier,concert_database_oid,concert_full_xid,
      concert_backend_pid,concert_backend_started,profile_epoch,profile_revision,
      profile_prepared_gid,profile_effect_digest)
      VALUES('{operation}','{actor}','{event}','{family}',decode(repeat('11',32),'hex'),
      decode(repeat('22',32),'hex'),decode(repeat('33',32),'hex'),decode('01','hex'),repeat('a',64),1,
      '{identity['system_identifier']}',{identity['database_oid']},'{identity['full_xid']}',
      {identity['pid']},'{identity['started']}','{epoch}',1,'{gid}',decode(repeat('44',32),'hex'));""")
    return str(operation), identity


def decide_sql(operation):
    return f"""UPDATE public.concert_participant_profile_operations SET
      decision='COMMIT',committed_revision=2,decided_at=clock_timestamp()
      WHERE operation_id='{operation}';"""


def denied_sql(sql):
    return "DO $$ BEGIN " + sql + " RAISE EXCEPTION 'expected constraint refusal'; EXCEPTION WHEN check_violation THEN NULL; END $$;"


def required_sources(repo):
    sources = list((repo / "backend/services/concert-service/src/participant").rglob("*.rs"))
    sources += list((repo / "backend/crates/platform/src").rglob("*.rs"))
    sources += list((repo / "backend/services/concert-service/src/adapters/out/relationship_authority/guarded").rglob("*.rs"))
    sources += [repo / relative for relative in (
        "backend/Cargo.lock", "backend/services/concert-service/Cargo.toml",
        "backend/services/concert-service/build.rs",
        "backend/proto/concert/v1/participant.proto", "backend/proto/authority/v1/lease.proto",
        "backend/crates/concert-application/src/avatar/mod.rs",
        "backend/services/concert-service/src/adapters/out/repository/avatar.rs",
        "backend/services/concert-service/migrations/0027_participant_profile_coordinator.sql")]
    return {str(path.relative_to(repo)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sources}


def validate_binary(binary, receipt, repo):
    pin = json.loads(receipt.read_text())
    actual = migration.executable(binary)
    require(actual == binary.resolve() and str(actual) == pin["copy"], "private artifact path mismatch")
    require(hashlib.sha256(actual.read_bytes()).hexdigest() == pin["sha256"], "artifact checksum mismatch")
    require(pin["compile_exit"] == 0 and pin["artifact"]["reason"] == "compiler-artifact"
            and pin["artifact"]["target"]["name"] == "concert_service"
            and pin["artifact"]["target"]["kind"] == ["lib"]
            and pin["artifact"]["profile"]["test"] is True
            and pin["artifact"]["manifest_path"] == str(repo / "backend/services/concert-service/Cargo.toml"), "exact Cargo test artifact required")
    emitted = Path(pin["artifact"]["executable"])
    require(emitted.is_absolute() and not actual.is_relative_to(emitted.parents[2]), "test must use a private copy outside the build target")
    require(pin["source_commit"] == subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(), "source commit changed")
    require(pin.get("source_sha256") == required_sources(repo), "required source pins differ")
    require(pin.get("rustc") == "1.88.0" and pin.get("incremental") == "0" and pin.get("debug") == "0"
            and pin.get("jobs") == 2 and pin.get("locked") is True and pin.get("offline") is True, "build constraints missing")
    return pin


def decision_endpoint_probe(cluster, binary, env, repo):
    """Two external processes using the real handler and dedicated key helpers.

    Node's built-in crypto derives public keys from fresh in-memory Ed25519 test
    seeds. No new package, fallback key, production secret or key file is used.
    """
    node = shutil.which("node")
    require(node is not None, "Node with built-in Ed25519 crypto required for endpoint probe")
    seeds = [base64.b64encode(os.urandom(32)).decode() for _ in range(2)]
    script = """const crypto=require('node:crypto');let input='';process.stdin.on('data',x=>input+=x);process.stdin.on('end',()=>{const keys=JSON.parse(input).map(seed=>{const key=crypto.createPrivateKey({key:Buffer.concat([Buffer.from('302e020100300506032b657004220420','hex'),Buffer.from(seed,'base64')]),format:'der',type:'pkcs8'});return crypto.createPublicKey(key).export({format:'der',type:'spki'}).subarray(-32).toString('base64')});process.stdout.write(JSON.stringify(keys))});"""
    derived = subprocess.run([node, "-e", script], input=json.dumps(seeds), text=True,
                             capture_output=True, check=True, timeout=5, env=cluster.env)
    keys = json.loads(derived.stdout)
    private_env = {**env, "ACTOR_DELEGATION_VERIFYING_KEYS": keys[0],
                   "PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS": keys[1],
                   "PROFILE_PARTICIPANT_DECISION_SIGNING_SEED": seeds[1],
                   "MCR_PARTICIPANT_DECISION_PROFILE_TOKEN": uuid.uuid4().hex}
    ready = cluster.root / "participant-decision-ready.json"
    stop = cluster.root / "participant-decision-stop"
    # Output stays in the private owned root; no key/token is logged.
    log = cluster.root / "participant-decision-host.log"
    fd = os.open(log, os.O_RDWR | os.O_CREAT | os.O_EXCL, 0o600)
    process = None
    with os.fdopen(fd, "w+") as output:
        try:
            process = subprocess.Popen([str(binary), "--ignored", "--exact", ENDPOINT_TEST, "--nocapture"],
                                       env=private_env, cwd=repo, stdout=output, stderr=subprocess.STDOUT)
            deadline = time.monotonic() + 5
            while not ready.exists():
                require(process.poll() is None, "actual decision endpoint failed before readiness")
                require(time.monotonic() < deadline, "decision readiness exceeded bound")
                time.sleep(0.025)
            endpoint_info = json.loads(ready.read_text())
            require(endpoint_info["pid"] == process.pid, "foreign decision endpoint readiness")
            result = migration.execute([str(binary), "--ignored", "--exact", CLIENT_TEST, "--nocapture"],
                                       env=private_env, phase="decision-client", cwd=repo, timeout=10)
            require("1 passed; 0 failed; 0 ignored" in result.stdout, "exact decision client test required")
            stop_fd = os.open(stop, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            os.close(stop_fd)
            require(process.wait(timeout=5) == 0, "decision endpoint did not stop successfully")
            output.seek(0)
            require("1 passed; 0 failed; 0 ignored" in output.read(), "exact decision host test required")
            require(not ready.exists(), "endpoint readiness was not retired")
            return {"state": "PASS", "host_test": ENDPOINT_TEST, "client_test": CLIENT_TEST,
                    "checks": ["dedicated signed unknown query stays unresolved", "no-store exact query echo",
                               "wrong workload credential denied", "foreign signed actor denied", "bounded clean endpoint shutdown"],
                    "limitation": "separate actual handler/client processes; no real Profile prepare or owner store join"}
        finally:
            if process is not None and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)


def run(pg_bin, binary=None, receipt=None):
    backend = Path(__file__).resolve().parents[2]
    repo = backend.parent
    require((binary is None) == (receipt is None), "binary and receipt must be supplied together")
    pin = validate_binary(binary, receipt, repo) if binary is not None else None
    require(shutil.disk_usage(repo).free >= 8 * 1024**3, "at least 8GiB free required")
    cluster = migration.Cluster({name: migration.executable(pg_bin / name) for name in migration.PG_TOOLS})
    report = {"state": "FAIL", "scope": "Concert coordinator/native decision half plus SQL constraints; explicit Profile/eligibility fixtures; no full Live acceptance" if pin else "0027 SQL-only native constraints; no Rust, guard or full Live acceptance", "checks": {},
              "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip(),
              "driver_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "cluster_helper_sha256": hashlib.sha256(Path(migration.__file__).read_bytes()).hexdigest()}
    sessions = []
    def interrupted(number, frame):
        raise migration.SmokeError("signal", str(number))
    previous = {number: signal.signal(number, interrupted) for number in (signal.SIGINT, signal.SIGTERM)}
    try:
        cluster.init()
        cluster.start()
        require(cluster.sql("postgres", "SHOW server_version_num;") == "160014", "exact PG16.14 required")
        cluster.sql("postgres", f"CREATE DATABASE {DATABASE} TEMPLATE template0;")
        report["migration_sha384"] = migration.migration_manifest(backend, "concert-service")
        for path in sorted((backend / "services/concert-service/migrations").glob("*.sql")):
            if not path.name.endswith(".down.sql"):
                cluster.sql(DATABASE, path.read_text())
        report["checks"]["all_current_concert_sql_applied"] = "PASS"
        if pin is not None:
            info = cluster.root.stat()
            env = {**cluster.env, "MCR_PARTICIPANT_COORDINATOR_ROOT": str(cluster.root),
                   "MCR_PARTICIPANT_COORDINATOR_IDENTITY": f"{info.st_dev}:{info.st_ino}",
                   "MCR_PARTICIPANT_COORDINATOR_TOKEN": cluster.token,
                   "MCR_PARTICIPANT_COORDINATOR_URL": cluster.url(DATABASE)}
            result = migration.execute([str(binary), "--ignored", "--exact", NATIVE_TEST, "--nocapture"],
                                       env=env, phase="native-coordinator", cwd=repo, timeout=30)
            require("1 passed; 0 failed; 0 ignored" in result.stdout, "exact native test did not pass")
            report["native_rust"] = {"test": NATIVE_TEST, "exit_code": result.returncode,
                                     "binary_sha256": pin["sha256"], "stdout": result.stdout}
            report["decision_endpoint"] = decision_endpoint_probe(cluster, binary, env, repo)
        cluster.sql(DATABASE, "CREATE TABLE public.coordinator_sql_effect_probe(operation_id uuid PRIMARY KEY);")

        committed = Session(cluster)
        sessions.append(committed)
        operation, identity = register(cluster, committed)
        cluster.sql(DATABASE, f"INSERT INTO public.concert_participant_profile_prepare_attempts(operation_id) VALUES('{operation}');")
        cluster.sql(DATABASE, denied_sql(f"UPDATE public.concert_participant_profile_prepare_attempts SET attempted_at=clock_timestamp() WHERE operation_id='{operation}';"))
        report["checks"]["prepare_attempt_is_durable_and_immutable"] = "PASS"
        # A second connection is not allowed to write the native decision.
        cluster.sql(DATABASE, denied_sql(decide_sql(operation)))
        require(cluster.sql(DATABASE, f"SELECT decision FROM public.concert_participant_profile_operations WHERE operation_id='{operation}';") == "PENDING", "wrong connection changed decision")
        report["checks"]["wrong_connection_cannot_decide"] = "PASS"
        committed.call(f"INSERT INTO public.coordinator_sql_effect_probe VALUES('{operation}');\n" + decide_sql(operation) + "\nCOMMIT;")
        require(cluster.sql(DATABASE, f"SELECT decision || ':' || committed_revision::text FROM public.concert_participant_profile_operations WHERE operation_id='{operation}';") == "COMMIT:2", "same-XID decision missing")
        require(cluster.sql(DATABASE, f"SELECT pg_xact_status('{identity['full_xid']}'::xid8);") == "committed", "commit status not authoritative")
        report["checks"]["same_native_transaction_effect_and_decision_commit"] = "PASS"
        committed.close()
        sessions.remove(committed)

        rolled = Session(cluster)
        sessions.append(rolled)
        aborted_operation, aborted_identity = register(cluster, rolled)
        rolled.call(f"INSERT INTO public.coordinator_sql_effect_probe VALUES('{aborted_operation}');\n" + decide_sql(aborted_operation) + "\nROLLBACK;")
        require(cluster.sql(DATABASE, f"SELECT decision FROM public.concert_participant_profile_operations WHERE operation_id='{aborted_operation}';") == "PENDING", "rollback leaked decision")
        require(cluster.sql(DATABASE, f"SELECT count(*) FROM public.coordinator_sql_effect_probe WHERE operation_id='{aborted_operation}';") == "0", "rollback leaked effect")
        require(cluster.sql(DATABASE, f"SELECT pg_xact_status('{aborted_identity['full_xid']}'::xid8);") == "aborted", "abort status not authoritative")
        report["checks"]["actual_rollback_preserves_registry_without_effect_or_decision"] = "PASS"
        rolled.close()
        sessions.remove(rolled)

        cluster.sql(DATABASE, denied_sql(f"UPDATE public.concert_participant_profile_operations SET body_digest=decode(repeat('55',32),'hex') WHERE operation_id='{aborted_operation}';"))
        cluster.sql(DATABASE, denied_sql(f"UPDATE public.concert_participant_profile_operations SET profile_prepared_gid='foreign' WHERE operation_id='{aborted_operation}';"))
        cluster.sql(DATABASE, denied_sql(f"UPDATE public.concert_participant_profile_operations SET decision='PENDING',committed_revision=NULL,decided_at=NULL WHERE operation_id='{operation}';"))
        report["checks"]["original_binding_profile_receipt_and_terminal_decision_immutable"] = "PASS"
        # SQL structural fixture ONLY: these metadata are explicitly synthetic
        # Profile receipt data, not a canonical owner attestation or live grant.
        cluster.sql(DATABASE, f"""INSERT INTO public.concert_participant_profile_terminal_receipts(
          operation_id,profile_prepared_gid,profile_effect_digest,outcome,
          profile_system_identifier,profile_database_oid,profile_full_xid,
          profile_backend_pid,profile_backend_started)
          SELECT operation_id,profile_prepared_gid,profile_effect_digest,'COMMITTED',
          concert_system_identifier,concert_database_oid,concert_full_xid,
          concert_backend_pid,concert_backend_started
          FROM public.concert_participant_profile_operations WHERE operation_id='{operation}';""")
        cluster.sql(DATABASE, denied_sql(f"UPDATE public.concert_participant_profile_terminal_receipts SET outcome='ABORTED' WHERE operation_id='{operation}';"))
        report["checks"]["terminal_receipt_structural_fixture_immutable"] = "PASS"
        cluster.stop()
        cluster.start()
        require(cluster.sql(DATABASE, f"SELECT decision FROM public.concert_participant_profile_operations WHERE operation_id='{operation}';") == "COMMIT", "restart lost decision")
        require(cluster.sql(DATABASE, f"SELECT count(*) FROM public.coordinator_sql_effect_probe WHERE operation_id='{operation}';") == "1", "restart lost effect")
        require(cluster.sql(DATABASE, f"SELECT pg_xact_status('{aborted_identity['full_xid']}'::xid8);") == "aborted", "restart lost actual abort outcome")
        require(cluster.sql(DATABASE, f"SELECT count(*) FROM public.concert_participant_profile_prepare_attempts WHERE operation_id='{operation}';") == "1", "restart lost prepare marker")
        require(cluster.sql(DATABASE, f"SELECT outcome FROM public.concert_participant_profile_terminal_receipts WHERE operation_id='{operation}';") == "COMMITTED", "restart lost terminal fixture")
        report["checks"]["restart_retains_decision_effect_and_abort_outcome"] = "PASS"
        report["state"] = "PASS"
    except (migration.SmokeError, OSError, ValueError, subprocess.SubprocessError) as error:
        report["error"] = error.detail if isinstance(error, migration.SmokeError) else str(error)
    finally:
        try:
            for session in sessions:
                session.close()
            cluster.cleanup()
            require(not cluster.root.exists(), "owned root remains")
            report["cleanup"] = "PASS"
        except (migration.SmokeError, OSError, subprocess.SubprocessError) as error:
            report["state"], report["cleanup"] = "FAIL", "FAIL"
            report["cleanup_error"] = str(error)
        for number, handler in previous.items():
            signal.signal(number, handler)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--receipt", type=Path)
    args = parser.parse_args()
    report = run(args.pg_bin, args.binary, args.receipt)
    print(json.dumps(report, sort_keys=True))
    return 0 if report["state"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
