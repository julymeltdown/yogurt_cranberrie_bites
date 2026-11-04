#!/usr/bin/env python3
"""Exercise only the durable scope SQL invariants in an owned PostgreSQL16 cluster.

No Cargo, external database, canonical owner RPC or browser flow is exercised.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import re
import migration_smoke as migration

ROOT = Path(__file__).resolve().parents[3]
MIGRATION = ROOT / "backend/services/concert-service/migrations/0028_product_owner_scope_lifecycle.sql"
GIFT_MIGRATION = ROOT / "backend/services/concert-service/migrations/0037_gift_private_child_intents.sql"
SOCIAL_MIGRATION = ROOT / "backend/services/concert-service/migrations/0042_social_read_child_intents.sql"
DATABASE = "mcr_product_scope_ledger"
INSTANCE = "a0280000-0000-4000-8000-000000000001"
SCOPE = "a0280000-0000-4000-8000-000000000002"
UNEXPOSED = "a0280000-0000-4000-8000-000000000003"
PARTIAL = "a0280000-0000-4000-8000-000000000004"
AUTH = "a0280000-0000-4000-8000-000000000005"
FRIEND = "a0280000-0000-4000-8000-000000000006"


def require(value, message):
    if not value:
        raise RuntimeError(message)


def run(args, report):
    tools = {name: migration.executable(args.pg_bin / name) for name in migration.PG_TOOLS}
    source_hash = hashlib.sha256(MIGRATION.read_bytes()).hexdigest()
    report["migration_sha256"] = source_hash
    gift_hash = hashlib.sha256(GIFT_MIGRATION.read_bytes()).hexdigest()
    report["gift_migration_sha256"] = gift_hash
    report["social_migration_sha256"] = hashlib.sha256(SOCIAL_MIGRATION.read_bytes()).hexdigest()
    binary = None
    if args.repository_test is not None:
        binary = migration.executable(args.repository_test)
        receipt = json.loads(args.artifact_receipt.read_text())
        artifact = receipt.get("cargo_artifact", {})
        require(receipt.get("exit_code") == 0 and receipt.get("locked") is True and receipt.get("offline") is True,
                "successful locked offline build receipt required")
        require(str(receipt.get("rustc_version", "")).startswith("rustc 1.88.0"), "Rust1.88 required")
        require(receipt.get("jobs") in (1, 2) and receipt.get("debug") == "0" and receipt.get("incremental") == "0",
                "bounded build receipt required")
        require(artifact.get("reason") == "compiler-artifact" and artifact.get("target", {}).get("name") == "product_scope_ledger"
                and artifact.get("profile", {}).get("test") is True, "exact Cargo test artifact required")
        require(Path(receipt.get("private_artifact", "")).resolve() == binary, "binary path differs")
        binary_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
        require(binary_hash == receipt.get("artifact_sha256"), "binary hash differs")
        pins = receipt.get("source_sha256", {})
        required = {ROOT / "backend/Cargo.lock", ROOT / "backend/Cargo.toml"}
        for directory in ("backend/services/concert-service", "backend/crates/platform",
                          "backend/crates/concert-application", "backend/crates/concert-domain"):
            required.update((ROOT / directory).rglob("*.rs"))
            required.add(ROOT / directory / "Cargo.toml")
        required.update((ROOT / "backend/proto").rglob("*.proto"))
        required.update((ROOT / "backend/services/concert-service/migrations").glob("*.sql"))
        require({str(path.relative_to(ROOT)) for path in required} <= pins.keys(), "source pins incomplete")
        for name, expected in pins.items():
            path = (ROOT / name).resolve(strict=True)
            require(not Path(name).is_absolute() and path.is_relative_to(ROOT), "invalid source pin")
            require(hashlib.sha256(path.read_bytes()).hexdigest() == expected, "source pin differs")
        report["artifact_sha256"] = binary_hash
        report["artifact_receipt_sha256"] = hashlib.sha256(args.artifact_receipt.read_bytes()).hexdigest()
    cluster = migration.Cluster(tools)
    try:
        cluster.init()
        cluster.start()
        report["postgres_version"] = cluster.sql("postgres", "SHOW server_version")
        require(report["postgres_version"].startswith("16."), "PostgreSQL16 required")
        cluster.sql("postgres", f"CREATE DATABASE {DATABASE} TEMPLATE template0")
        result = subprocess.run(
            [str(tools["psql"]), "-X", "--no-password", "-v", "ON_ERROR_STOP=1", "-1", "-f", str(MIGRATION)],
            env=cluster.client_env(DATABASE), cwd=cluster.root, capture_output=True, timeout=30)
        report["commands"]["apply-migration"] = result.returncode
        require(result.returncode == 0, "scope migration failed")
        result = subprocess.run(
            [str(tools["psql"]), "-X", "--no-password", "-v", "ON_ERROR_STOP=1", "-1", "-f", str(GIFT_MIGRATION)],
            env=cluster.client_env(DATABASE), cwd=cluster.root, capture_output=True, timeout=30)
        report["commands"]["apply-gift-child-migration"] = result.returncode
        require(result.returncode == 0, "Gift child kind migration failed")
        result = subprocess.run(
            [str(tools["psql"]), "-X", "--no-password", "-v", "ON_ERROR_STOP=1", "-1", "-f", str(SOCIAL_MIGRATION)],
            env=cluster.client_env(DATABASE), cwd=cluster.root, capture_output=True, timeout=30)
        report["commands"]["apply-social-child-migration"] = result.returncode
        require(result.returncode == 0, "Social child kind migration failed")
        if binary is not None:
            env = {**cluster.env, "CONCERT_TEST_DATABASE_URL": cluster.url(DATABASE),
                   "CONCERT_TEST_OWNER_ROOT": str(cluster.root), "CONCERT_TEST_OWNER_TOKEN": cluster.token,
                   "CONCERT_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}"}
            native = subprocess.run([str(binary), "--test-threads=1"], cwd=cluster.root, env=env,
                                    capture_output=True, timeout=45)
            output = (native.stdout + native.stderr).decode(errors="replace")
            require(all(value not in output for value in (cluster.password, cluster.token, cluster.url(DATABASE))),
                    "native diagnostic secret leak")
            summary = re.search(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", output)
            require(summary is not None, "native test summary missing")
            report["native_tests"] = dict(zip(("passed", "failed", "ignored"), map(int, summary.groups())))
            report["commands"]["native-rust-ledger"] = native.returncode
            report["native_diagnostic"] = migration.redact(output, (cluster.password, cluster.token))[-5000:]
            require(native.returncode == 0 and report["native_tests"]["passed"] >= 3
                    and report["native_tests"]["failed"] == 0 and report["native_tests"]["ignored"] == 0,
                    "native Rust ledger failed")
            require(hashlib.sha256(binary.read_bytes()).hexdigest() == binary_hash, "native binary changed")
            report["rust_ledger"] = "PASS"
        key = f"instance_id='{INSTANCE}' AND scope_id='{SCOPE}'"

        def sql(query):
            return cluster.sql(DATABASE, query)

        def denied(label, query):
            result = subprocess.run(
                [str(tools["psql"]), "-X", "--no-password", "-v", "ON_ERROR_STOP=1", "-c", query],
                env=cluster.client_env(DATABASE), cwd=cluster.root, capture_output=True, timeout=10)
            require(result.returncode != 0, f"expected refusal: {label}")
            report["commands"][label] = result.returncode

        def insert_scope(scope):
            sql(f"INSERT INTO concert_product_owner_scopes(instance_id,scope_id,binding_digest,binding) "
                f"VALUES('{INSTANCE}','{scope}',decode(repeat('11',32),'hex'),'{{}}')")

        def insert_transaction(scope, byte):
            sql(f"INSERT INTO concert_product_owner_transactions(instance_id,scope_id,binding_digest,identity_digest,identity) "
                f"VALUES('{INSTANCE}','{scope}',decode(repeat('11',32),'hex'),decode(repeat('{byte}',32),'hex'),'{{}}')")

        insert_scope(SCOPE)
        denied("active-cannot-finish", f"UPDATE concert_product_owner_scopes SET phase='ROOT_FINISHED',root_finished_at=clock_timestamp() WHERE {key}")
        denied("binding-immutable", f"UPDATE concert_product_owner_scopes SET binding='{{\"changed\":true}}' WHERE {key}")
        denied("serialization-needs-delivery", f"UPDATE concert_product_owner_scopes SET serialized_at=clock_timestamp() WHERE {key}")
        sql(f"UPDATE concert_product_owner_scopes SET delivery_secret_hash=decode(repeat('22',32),'hex') WHERE {key}")
        denied("delivery-cannot-clear", f"UPDATE concert_product_owner_scopes SET delivery_secret_hash=NULL WHERE {key}")
        denied("delivery-cannot-rotate", f"UPDATE concert_product_owner_scopes SET delivery_secret_hash=decode(repeat('33',32),'hex') WHERE {key}")
        insert_transaction(SCOPE, "44")
        sql(f"UPDATE concert_product_owner_transactions SET auth_registration_id='{AUTH}' WHERE {key}")
        denied("partial-tracking-cannot-confirm", f"UPDATE concert_product_owner_transactions SET confirmed_at=clock_timestamp() WHERE {key}")
        denied("tracking-cannot-relabel", f"UPDATE concert_product_owner_transactions SET auth_registration_id='{FRIEND}' WHERE {key}")
        sql(f"UPDATE concert_product_owner_transactions SET friend_registration_id='{FRIEND}' WHERE {key}")
        sql(f"UPDATE concert_product_owner_transactions SET confirmed_at=clock_timestamp() WHERE {key}")
        denied("confirmed-transaction-immutable", f"UPDATE concert_product_owner_transactions SET identity='{{\"changed\":true}}' WHERE {key}")
        child = f"instance_id='{INSTANCE}' AND scope_id='{SCOPE}'"
        child_insert = ("INSERT INTO concert_product_child_intents(instance_id,scope_id,binding_digest,kind,intent_id,key_id,sealed_reference,reference_digest) "
                        f"VALUES('{INSTANCE}','{SCOPE}',decode(repeat('11',32),'hex'),'PROFILE_PUBLIC','a0280000-0000-4000-8000-000000000008','fixture',decode(repeat('77',28),'hex'),decode(repeat('88',32),'hex'))")
        sql(child_insert)
        sql(child_insert.replace("PROFILE_PUBLIC", "GIFT_PRIVATE").replace("000000000008", "000000000010"))
        require(sql(f"SELECT count(*) FROM concert_product_child_intents WHERE {child} AND kind='GIFT_PRIVATE'") == "1",
                "closed Gift child kind not admitted")
        report["commands"]["closed-gift-child-kind"] = 0
        sql(child_insert.replace("PROFILE_PUBLIC", "SOCIAL_READ").replace("000000000008", "000000000012"))
        require(sql(f"SELECT count(*) FROM concert_product_child_intents WHERE {child} AND kind='SOCIAL_READ'") == "1",
                "closed Social child kind not admitted")
        report["commands"]["closed-social-child-kind"] = 0
        denied("unknown-child-kind-denied", child_insert.replace("PROFILE_PUBLIC", "GIFT_ARBITRARY").replace("000000000008", "000000000011"))
        denied("child-cannot-complete-before-root", f"UPDATE concert_product_child_intents SET completed_at=clock_timestamp() WHERE {child}")
        denied("child-cannot-rebind", f"UPDATE concert_product_child_intents SET key_id='other' WHERE {child}")
        denied("child-evidence-cannot-delete", f"DELETE FROM concert_product_child_intents WHERE {child}")
        sql(f"UPDATE concert_product_owner_scopes SET serialized_at=clock_timestamp() WHERE {key}")
        require(sql(f"SELECT phase FROM concert_product_owner_scopes WHERE {key}") == "ACTIVE", "serialization finished scope early")
        report["commands"]["serialization-does-not-finish"] = 0
        sql(f"UPDATE concert_product_owner_scopes SET phase='SEALED',sealed_at=clock_timestamp() WHERE {key}")
        denied("late-transaction-denied", f"INSERT INTO concert_product_owner_transactions(instance_id,scope_id,binding_digest,identity_digest,identity) VALUES('{INSTANCE}','{SCOPE}',decode(repeat('11',32),'hex'),decode(repeat('55',32),'hex'),'{{}}')")
        denied("late-child-denied", child_insert.replace("000000000008", "000000000009"))
        denied("sealed-cannot-reopen", f"UPDATE concert_product_owner_scopes SET phase='ACTIVE',sealed_at=NULL WHERE {key}")
        sql(f"UPDATE concert_product_owner_scopes SET phase='ROOT_FINISHED',root_finished_at=clock_timestamp() WHERE {key}")
        denied("delivery-needs-all-children", f"UPDATE concert_product_owner_scopes SET delivery_completed_at=clock_timestamp() WHERE {key}")
        denied("finished-cannot-reopen", f"UPDATE concert_product_owner_scopes SET phase='ACTIVE',sealed_at=NULL,root_finished_at=NULL WHERE {key}")
        denied("scope-evidence-cannot-delete", f"DELETE FROM concert_product_owner_scopes WHERE {key}")
        denied("transaction-evidence-cannot-delete", f"DELETE FROM concert_product_owner_transactions WHERE {key}")
        insert_scope(UNEXPOSED)
        unexposed = f"instance_id='{INSTANCE}' AND scope_id='{UNEXPOSED}'"
        sql(f"UPDATE concert_product_owner_scopes SET phase='SEALED',sealed_at=clock_timestamp() WHERE {unexposed}")
        sql(f"UPDATE concert_product_owner_scopes SET phase='ROOT_FINISHED',root_finished_at=clock_timestamp() WHERE {unexposed}")
        report["commands"]["unexposed-acquisition-cleanup"] = 0
        insert_scope(PARTIAL)
        insert_transaction(PARTIAL, "66")
        partial = f"instance_id='{INSTANCE}' AND scope_id='{PARTIAL}'"
        sql(f"UPDATE concert_product_owner_transactions SET auth_registration_id='a0280000-0000-4000-8000-000000000007' WHERE {partial}")
        cluster.stop()
        cluster.start()
        require(sql(f"SELECT phase FROM concert_product_owner_scopes WHERE {key}") == "ROOT_FINISHED", "finished state lost")
        require(sql(f"SELECT count(*)=3 AND bool_and(completed_at IS NULL AND key_id='fixture' AND sealed_reference=decode(repeat('77',28),'hex')) FROM concert_product_child_intents WHERE {child}") == "t", "unresolved child evidence lost")
        report["commands"]["restart-retains-child-intent"] = 0
        sql(f"UPDATE concert_product_child_intents SET completed_at=clock_timestamp() WHERE {child}")
        denied("completed-child-cannot-reopen", f"UPDATE concert_product_child_intents SET completed_at=NULL WHERE {child}")
        denied("completed-child-cannot-change-ciphertext", f"UPDATE concert_product_child_intents SET sealed_reference=decode(repeat('99',28),'hex') WHERE {child}")
        sql(f"UPDATE concert_product_owner_scopes SET delivery_completed_at=clock_timestamp() WHERE {key}")
        require(sql(f"SELECT delivery_completed_at IS NOT NULL FROM concert_product_owner_scopes WHERE {key}") == "t", "settled child blocked durable completion")
        report["commands"]["settled-child-allows-completion"] = 0
        require(sql(f"SELECT phase FROM concert_product_owner_scopes WHERE {partial}") == "ACTIVE", "unfinished scope falsely settled")
        require(sql(f"SELECT auth_registration_id IS NOT NULL AND friend_registration_id IS NULL AND confirmed_at IS NULL FROM concert_product_owner_transactions WHERE {partial}") == "t", "partial owner tracking lost")
        require(sql("SELECT count(*) FROM concert_product_owner_scopes WHERE scope_id='a0280000-0000-4000-8000-000000000099' AND phase='ROOT_FINISHED'") == "0", "missing scope falsely settled")
        report["commands"]["restart-and-unknown-retention"] = 0
        require(hashlib.sha256(MIGRATION.read_bytes()).hexdigest() == source_hash, "migration changed during probe")
        require(hashlib.sha256(GIFT_MIGRATION.read_bytes()).hexdigest() == gift_hash, "Gift migration changed during probe")
        report["result"] = "PASS"
    finally:
        cluster.cleanup()
        require(not cluster.root.exists(), "owned cluster remains")
        report["cleanup"] = "PASS"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pg-bin", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--repository-test", type=Path)
    parser.add_argument("--artifact-receipt", type=Path)
    args = parser.parse_args()
    if not args.pg_bin.is_absolute() or not args.report.is_absolute():
        parser.error("absolute paths required")
    if (args.repository_test is None) != (args.artifact_receipt is None):
        parser.error("native binary and receipt must be provided together")
    if args.repository_test is not None and not all(path.is_absolute() for path in (args.repository_test, args.artifact_receipt)):
        parser.error("absolute native binary and receipt paths required")
    fd = os.open(args.report, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    report = {"result": "FAIL", "scope": "durable scope SQL invariants only", "commands": {},
              "cleanup": "NOT_RUN", "canonical_owner_rpc": "NOT_RUN", "gateway_http": "NOT_RUN",
              "rust_ledger": "NOT_RUN"}
    handlers = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            handlers[sig] = signal.signal(sig, interrupted)
        run(args, report)
    except (Exception, KeyboardInterrupt) as error:
        report["failure"] = type(error).__name__
    finally:
        for sig, handler in handlers.items():
            signal.signal(sig, handler)
        with os.fdopen(fd, "w") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    print(json.dumps({"result": report["result"], "cleanup": report["cleanup"], "checks": len(report["commands"])}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
