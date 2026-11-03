#!/usr/bin/env python3
"""Qualify BE08 in a new marker-owned PostgreSQL cluster; no product API."""
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
DATABASE = "mcr_places"
MIGRATIONS = (
    "0001_concert_core.sql",
    "0002_command_receipts_outbox.sql",
    "0003_places_map_releases.sql",
)


class Failure(Exception):
    pass


def require(value: bool, label: str) -> None:
    if not value:
        raise Failure(label)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical(value: object) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=True,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("ascii")


def apply_files(cluster: migration.Cluster) -> None:
    for name in MIGRATIONS:
        path = ROOT / "backend/services/concert-service/migrations" / name
        require(path.is_file(), "migration-missing")
        cluster.sql(DATABASE, path.read_text())


def assert_schema(cluster: migration.Cluster) -> None:
    tables = (
        "concert_map_releases",
        "concert_map_release_places",
        "concert_events_map_releases",
        "concert_place_current_state",
    )
    query = " and ".join(
        f"to_regclass('public.{table}') is not null" for table in tables
    )
    require(cluster.sql(DATABASE, query) == "t", "schema-missing")


def literal(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def seed_fixture(cluster: migration.Cluster) -> tuple[dict, bytes, str]:
    source = (ROOT / "contracts/fixtures/assets/map-manifest.json").read_bytes()
    value = json.loads(source)
    encoded = canonical(value)
    without_hash = dict(value)
    without_hash.pop("content_hash")
    require(
        value["content_hash"]
        == "sha256:" + hashlib.sha256(canonical(without_hash)).hexdigest(),
        "fixture-content-hash",
    )
    require(
        value["geometry_hash"]
        == "sha256:" + hashlib.sha256(canonical(value["geometry"])).hexdigest(),
        "fixture-geometry-hash",
    )
    event = "11111111-1111-4111-8111-111111111111"
    cluster.sql(
        DATABASE,
        "insert into concert_events(event_id,slug,title,phase) "
        f"values('{event}','places-owned','Owned places','OPEN');",
    )
    cluster.sql(
        DATABASE,
        "insert into concert_map_releases("
        "map_id,map_revision,content_hash,geometry_hash,canonical_bytes) values("
        f"'map_fixture',1,{literal(value['content_hash'])},"
        f"{literal(value['geometry_hash'])},decode('{encoded.hex()}','hex'));",
    )
    place = value["places"][0]
    provenance = json.dumps(place["provenance"], separators=(",", ":"))
    cluster.sql(
        DATABASE,
        "insert into concert_map_release_places("
        "map_id,map_revision,place_id,label,place_kind,verification_state,"
        "accessibility_notes,source_kind,source_uri,review_state,provenance) values("
        f"'map_fixture',1,{literal(place['place_id'])},"
        f"{literal(place['label'])},{literal(place['place_kind'])},"
        f"{literal(place['verification_state'])},"
        f"{literal(place['accessibility_notes'])},"
        f"{literal(place['provenance']['source_kind'])},null,"
        f"{literal(place['provenance']['review_state'])},{literal(provenance)});",
    )
    cluster.sql(
        DATABASE,
        "insert into concert_events_map_releases("
        "event_id,catalog_revision,map_id,map_revision) "
        f"values('{event}',1,'map_fixture',1);",
    )
    cluster.sql(
        DATABASE,
        "insert into concert_place_current_state("
        "map_id,map_revision,place_id,closure_state) values("
        f"'map_fixture',1,{literal(place['place_id'])},'OPEN');",
    )
    require(
        cluster.sql(
            DATABASE,
            "select count(*) from concert_map_release_places "
            "where map_id='map_fixture' and map_revision=1;",
        )
        == "1",
        "fixture-place",
    )
    return value, encoded, event


def expect_refused(cluster: migration.Cluster, sql: str, label: str) -> None:
    try:
        cluster.sql(DATABASE, sql)
    except migration.SmokeError:
        return
    raise Failure(label)


def exercise(
    cluster: migration.Cluster,
    binary: Path,
    report: dict,
) -> None:
    cluster.init()
    cluster.start()
    report["postgres_version"] = cluster.sql("postgres", "show server_version;")
    cluster.sql("postgres", f"create database {DATABASE} template template0;")
    apply_files(cluster)
    assert_schema(cluster)
    report["direct_sql_migrations"] = "PASS"
    value, encoded, event = seed_fixture(cluster)
    place = value["places"][0]
    report["fixture_place_id"] = place["place_id"]
    expect_refused(
        cluster,
        "update concert_map_releases set canonical_bytes=canonical_bytes||' ' "
        "where map_id='map_fixture' and map_revision=1;",
        "immutable-release-update-not-refused",
    )
    expect_refused(
        cluster,
        "delete from concert_map_releases "
        "where map_id='map_fixture' and map_revision=1;",
        "immutable-release-delete-not-refused",
    )
    expect_refused(
        cluster,
        "update concert_map_release_places set label='changed' "
        "where map_id='map_fixture' and map_revision=1;",
        "immutable-release-place-update-not-refused",
    )
    expect_refused(
        cluster,
        "delete from concert_map_release_places "
        "where map_id='map_fixture' and map_revision=1;",
        "immutable-release-place-delete-not-refused",
    )
    report["immutable_release"] = "PASS"
    cluster.sql(
        DATABASE,
        "update concert_place_current_state set closure_state='CLOSED',"
        "revision=revision+1 "
        f"where map_id='map_fixture' and map_revision=1 "
        f"and place_id={literal(place['place_id'])};",
    )
    require(
        cluster.sql(
            DATABASE,
            "select closure_state from concert_place_current_state "
            f"where map_id='map_fixture' and map_revision=1 "
            f"and place_id={literal(place['place_id'])};",
        )
        == "CLOSED",
        "closure-update",
    )
    require(
        cluster.sql(
            DATABASE,
            "select encode(canonical_bytes,'hex') from concert_map_releases "
            "where map_id='map_fixture' and map_revision=1;",
        )
        == encoded.hex(),
        "closure-changed-release-bytes",
    )
    report["closure_independent"] = "PASS"
    expect_refused(
        cluster,
        "update concert_place_current_state set closure_state='OPEN' "
        f"where map_id='map_fixture' and map_revision=1 "
        f"and place_id={literal(place['place_id'])};",
        "current-state-revision-not-refused",
    )
    expect_refused(
        cluster,
        "delete from concert_place_current_state "
        f"where map_id='map_fixture' and map_revision=1 "
        f"and place_id={literal(place['place_id'])};",
        "current-state-delete-not-refused",
    )
    try:
        cluster.sql(
            DATABASE,
            "begin; insert into concert_events_map_releases("
            "event_id,catalog_revision,map_id,map_revision) values("
            f"'{event}',2,'map_fixture',2); rollback;",
        )
    except migration.SmokeError:
        pass
    else:
        raise Failure("missing-map-binding-not-refused")
    try:
        apply_files(cluster)
    except migration.SmokeError:
        report["direct_sql_reapply"] = "EXPECTED_REFUSED"
    else:
        raise Failure("direct-sql-reapply-not-refused")

    environment = {
        **cluster.env,
        "PLACES_TEST_DATABASE_URL": cluster.url(DATABASE),
        "PLACES_TEST_OWNER_ROOT": str(cluster.root),
        "PLACES_TEST_OWNER_TOKEN": cluster.token,
        "PLACES_TEST_OWNER_IDENTITY": (
            f"{cluster.identity[0]}:{cluster.identity[1]}"
        ),
    }
    result = subprocess.run(
        [str(binary), "--test-threads=1"],
        env=environment,
        cwd=cluster.root,
        capture_output=True,
        text=True,
        timeout=60,
    )
    output = result.stdout + result.stderr
    report["native_test_exit"] = result.returncode
    report["native_test_output_sha256"] = hashlib.sha256(output.encode()).hexdigest()
    require(
        cluster.password not in output and cluster.url(DATABASE) not in output,
        "secret-leak",
    )
    require(result.returncode == 0, "native-place-test")
    expected_closure = cluster.sql(
        DATABASE,
        "select closure_state from concert_place_current_state "
        f"where map_id='map_fixture' and map_revision=1 "
        f"and place_id={literal(place['place_id'])};",
    )
    require(expected_closure == "CLOSED", "closure-during-native-test")
    release_rows = cluster.sql(
        DATABASE,
        "select count(*) from concert_map_release_places "
        "where map_id='map_fixture' and map_revision=2;",
    )
    require(release_rows == "1", "native-release-membership")
    binding_count = cluster.sql(
        DATABASE,
        "select count(*) from concert_events_map_releases "
        "where event_id='11111111-1111-4111-8111-111111111111';"
    )
    require(binding_count == "1", "native-event-isolation")
    before = cluster.assert_postmaster()
    cluster.stop()
    cluster.start()
    require(cluster.assert_postmaster() != before, "restart-identity")
    require(
        cluster.sql(
            DATABASE,
            "select closure_state from concert_place_current_state "
            "where map_id='map_fixture' and map_revision=1 "
            f"and place_id={literal(place['place_id'])};",
        )
        == expected_closure,
        "restart-persistence",
    )
    require(
        cluster.sql(
            DATABASE,
            "select encode(canonical_bytes,'hex') from concert_map_releases "
            "where map_id='map_fixture' and map_revision=1;",
        )
        == encoded.hex(),
        "restart-release-bytes",
    )
    report["restart_persistence"] = "PASS"


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
    except (Exception, KeyboardInterrupt) as error:
        report["result"] = "FAIL"
        report["failure"] = (
            str(error) if isinstance(error, Failure) else type(error).__name__
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
        fd = os.open(
            args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600
        )
    except OSError:
        parser.error("report must be a new writable file")
    sources = [
        ROOT / "backend/crates/concert-domain/src/place/mod.rs",
        ROOT / "backend/crates/concert-application/src/place/mod.rs",
        ROOT / "backend/services/concert-service/src/adapters/out/repository/place.rs",
        ROOT / "backend/services/concert-service/migrations/0003_places_map_releases.sql",
    ]
    report = {
        "scope": "BE08 owned places and immutable map releases",
        "result": "FAIL",
        "cleanup": "NOT_RUN",
        "production": "NOT_RUN",
        "public_routes": "NOT_EXPOSED",
        "migration_ledger": "NOT_APPLIED",
        "source_revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip(),
        "source_sha256": {
            str(path.relative_to(ROOT)): digest(path) for path in sources
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
