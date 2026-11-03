#!/usr/bin/env python3
"""Qualify BE10/BE11 against a new marker-owned, socket-only PostgreSQL cluster."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess

import migration_smoke as migration


ROOT = Path(__file__).resolve().parents[3]
DATABASE = "mcr_world"
MIGRATIONS = (
    "0001_concert_core.sql",
    "0002_command_receipts_outbox.sql",
    "0003_places_map_releases.sql",
    "0005_world_admission.sql",
)
TABLES = (
    "concert_world_rooms",
    "concert_world_owner_leases",
    "concert_world_tickets",
    "concert_world_memberships",
)
ADMISSION_SOURCES = (
    "backend/crates/concert-application/src/world_ticket/mod.rs",
    "backend/services/concert-service/src/world/mod.rs",
    "backend/services/concert-service/migrations/0005_world_admission.sql",
)
GATEWAY_SOURCES = (
    "backend/services/api-gateway/src/adapters/in/http/concert_world.rs",
    "backend/services/api-gateway/src/adapters/out/concert_world.rs",
    "backend/services/api-gateway/tests/concert_identity_composition.rs",
    "backend/services/api-gateway/tests/support/concert_world_client.rs",
    "backend/services/api-gateway/tests/support/concert_world_http.rs",
    "backend/services/concert-service/src/bootstrap/mod.rs",
    "backend/services/concert-service/src/main.rs",
    "backend/services/concert-service/src/world/rpc.rs",
    "backend/services/concert-service/src/world_rpc.rs",
    "backend/services/concert-service/tests/world_rpc_authority.rs",
)


class Failure(Exception):
    pass


def require(value: bool, label: str) -> None:
    if not value:
        raise Failure(label)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def apply_files(cluster: migration.Cluster) -> None:
    for name in MIGRATIONS:
        path = ROOT / "backend/services/concert-service/migrations" / name
        require(path.is_file(), "migration-missing")
        cluster.sql(DATABASE, path.read_text())


def assert_schema(cluster: migration.Cluster) -> None:
    present = " and ".join(
        f"to_regclass('public.{table}') is not null" for table in TABLES
    )
    require(cluster.sql(DATABASE, f"select {present};") == "t", "schema-missing")
    digest_type = cluster.sql(
        DATABASE,
        "select data_type from information_schema.columns "
        "where table_schema='public' and table_name='concert_world_tickets' "
        "and column_name='ticket_digest';",
    )
    require(digest_type == "bytea", "ticket-digest-not-bytea")
    raw_columns = cluster.sql(
        DATABASE,
        "select count(*) from information_schema.columns "
        "where table_schema='public' and table_name in ("
        "'concert_world_tickets','concert_world_memberships') "
        "and column_name in ('ticket','raw_ticket','access_token','authorization');",
    )
    require(raw_columns == "0", "raw-secret-column-present")


def seed(cluster: migration.Cluster) -> None:
    cluster.sql(
        DATABASE,
        "insert into concert_events(event_id,slug,title,phase) values("
        "'11111111-1111-4111-8111-111111111111','world-owned','Owned world','OPEN');",
    )
    cluster.sql(
        DATABASE,
        "insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,"
        "avatar_manifest_id,avatar_manifest_version,is_default) "
        "values('11111111-1111-4111-8111-111111111111',"
        "'22222222-2222-4222-8222-222222222222','Main',50,'map_fixture',1,"
        "'avatar_manifest',1,true);",
    )


def expect_refused(cluster: migration.Cluster, sql: str, label: str) -> None:
    try:
        cluster.sql(DATABASE, sql)
    except migration.SmokeError:
        return
    raise Failure(label)


def exercise(cluster: migration.Cluster, binary: Path, report: dict) -> None:
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    cluster.sql("postgres", f"create database {DATABASE} template template0;")
    apply_files(cluster)
    assert_schema(cluster)
    seed(cluster)
    report["direct_sql_migrations"] = "PASS"

    # These constraints must reject invalid state regardless of the Rust adapter.
    expect_refused(
        cluster,
        "update concert_world_rooms set capacity=51 "
        "where event_id='11111111-1111-4111-8111-111111111111' "
        "and room_id='22222222-2222-4222-8222-222222222222';",
        "room-capacity-ceiling",
    )
    expect_refused(
        cluster,
        "insert into concert_world_tickets(ticket_digest,user_id,session_id,event_id,"
        "room_id,map_id,map_revision,avatar_manifest_id,avatar_manifest_version,"
        "issued_at,expires_at,access_expires_at) values("
        "decode('00','hex'),'33333333-3333-4333-8333-333333333333',"
        "'44444444-4444-4444-8444-444444444444',"
        "'11111111-1111-4111-8111-111111111111',"
        "'22222222-2222-4222-8222-222222222222','map',1,'avatar',1,"
        "statement_timestamp(),statement_timestamp()+interval '1 second',"
        "statement_timestamp()+interval '2 seconds');",
        "ticket-digest-width",
    )
    expect_refused(
        cluster,
        "insert into concert_world_rooms(event_id,room_id,label,capacity,map_id,map_revision,"
        "avatar_manifest_id,avatar_manifest_version,is_default) "
        "values('11111111-1111-4111-8111-111111111111',"
        "'55555555-5555-4555-8555-555555555555','Second default',50,'map_fixture',1,"
        "'avatar_manifest',1,true);",
        "one-default-room",
    )
    expect_refused(
        cluster,
        "insert into concert_world_owner_leases(event_id,room_id,owner_process_id,"
        "owner_epoch,leased_at,lease_until) values("
        "'11111111-1111-4111-8111-111111111111',"
        "'99999999-9999-4999-8999-999999999999','foreign',"
        "'88888888-8888-4888-8888-888888888888',statement_timestamp(),"
        "statement_timestamp()+interval '1 second');",
        "owner-room-scope",
    )
    report["schema_constraints"] = "PASS"

    environment = {
        **cluster.env,
        "WORLD_TEST_DATABASE_URL": cluster.url(DATABASE),
        "WORLD_TEST_OWNER_ROOT": str(cluster.root),
        "WORLD_TEST_OWNER_TOKEN": cluster.token,
        "WORLD_TEST_OWNER_IDENTITY": (
            f"{cluster.identity[0]}:{cluster.identity[1]}"
        ),
    }
    result = subprocess.run(
        [str(binary), "--test-threads=1"],
        env=environment,
        cwd=cluster.root,
        capture_output=True,
        text=True,
        timeout=120,
    )
    output = result.stdout + result.stderr
    report["native_test_exit"] = result.returncode
    report["native_test_output_sha256"] = hashlib.sha256(output.encode()).hexdigest()
    if result.returncode != 0:
        report["native_test_tail"] = output[-4000:]
    require(
        cluster.password not in output
        and cluster.url(DATABASE) not in output
        and cluster.token not in output,
        "secret-leak",
    )
    require(result.returncode == 0, "native-world-test")

    lease_count = cluster.sql(
        DATABASE,
        "select count(*) from concert_world_owner_leases;",
    )
    require(int(lease_count) >= 1, "persisted-owner-lease")
    consumer_count = cluster.sql(
        DATABASE,
        "select count(*) from concert_world_tickets where consumed_at is not null;",
    )
    require(int(consumer_count) >= 1, "persisted-ticket-consumption")

    before = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(cluster.assert_postmaster() != before, "restart-identity")
    require(
        int(
            cluster.sql(
                DATABASE,
                "select count(*) from concert_world_owner_leases;",
            )
        )
        >= 1,
        "restart-owner-persistence",
    )
    report["restart_persistence"] = "PASS"

    try:
        apply_files(cluster)
    except migration.SmokeError:
        report["direct_sql_reapply"] = "EXPECTED_REFUSED"
    else:
        raise Failure("direct-sql-reapply-not-refused")


def run(binary: Path, report: dict) -> None:
    cluster = None
    try:
        require(binary.is_absolute(), "explicit-binary")
        binary = migration.executable(binary)
        pg_tools = {
            name: migration.executable(Path("/opt/homebrew/bin") / name)
            for name in migration.PG_TOOLS
        }
        cluster = migration.Cluster(pg_tools)
        report["owned_directory"] = str(cluster.root)
        report["binary_sha256"] = digest(binary)
        exercise(cluster, binary, report)
        report["result"] = "PASS"
    except Failure:
        raise
    except (Exception, KeyboardInterrupt) as error:
        report["result"] = "FAIL"
        report["failure"] = (
            str(error)
            if isinstance(error, Failure)
            else getattr(error, "detail", type(error).__name__)
        )
    finally:
        try:
            if cluster is not None:
                cluster.cleanup()
            report["cleanup"] = "PASS"
        except BaseException:
            report["cleanup"] = "FAIL"
            report["result"] = "FAIL"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args(argv)
    require(args.report.is_absolute(), "absolute-report")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be a new writable file")
    sources = [ROOT / path for path in ADMISSION_SOURCES]
    report = {
        "scope": "BE10/BE11 application contracts and durable PostgreSQL admission",
        "result": "FAIL",
        "cleanup": "NOT_RUN",
        "production": "NOT_RUN",
        "http_ingress": "NOT_IMPLEMENTED",
        "websocket_ingress": "NOT_IMPLEMENTED",
        "be12_simulation": "NOT_IMPLEMENTED",
        "be13_privacy_projection": "NOT_IMPLEMENTED",
        "source_revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip(),
        "source_sha256": {
            str(path.relative_to(ROOT)): digest(path) for path in sources
        },
        "gateway_http_source_sha256": {
            path: digest(ROOT / path) for path in GATEWAY_SOURCES
        },
    }
    previous = signal.signal(
        signal.SIGTERM, lambda *_: (_ for _ in ()).throw(KeyboardInterrupt())
    )
    try:
        report["script_sha256"] = digest(Path(__file__))
        run(args.binary, report)
    except BaseException as error:
        report["result"] = "FAIL"
        report["failure"] = type(error).__name__
    finally:
        signal.signal(signal.SIGTERM, previous)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(
        json.dumps(
            {
                key: report.get(key)
                for key in (
                    "result",
                    "cleanup",
                    "native_test_exit",
                    "restart_persistence",
                )
            }
        )
    )
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
