#!/usr/bin/env python3
"""Actual public USER/no-media Report journey in the existing owned seven-service stack.

No service starts on import. CLI allocation, binary custody, migrations, TLS,
three real signup/verify/login/C03 principals and cleanup use the pinned launcher.
Only Report-specific configuration and the synchronous HTTP journey are added.
"""
from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import hashlib
import http.client
import importlib.util
import json
import os
from pathlib import Path
import secrets
import select
import subprocess
import time
from types import SimpleNamespace
from unittest.mock import patch
from uuid import UUID, uuid4

import in03_stack_runtime as launcher

ROOT = Path(__file__).resolve().parents[3]
require = launcher.require
STACK_RUN = launcher.run
FAILURE_COUNTS = frozenset(("scopes", "active", "sealed", "root_finished", "serialized",
    "delivery_completed", "response_abandoned", "cancellation_settled", "registered_native",
    "confirmed_native", "operations_witnesses", "child_intents", "pending_children"))
REPORT_OPERATIONS = frozenset(("createConcertReport", "listOwnConcertReports", "getConcertCommandStatus"))


def contract():
    spec = importlib.util.spec_from_file_location("mcr_own_reports_c04", ROOT / "scripts/mcr/validate-domain.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    value = module.Contract()
    formats = set()

    def visit(item):
        if isinstance(item, dict):
            if isinstance(item.get("format"), str):
                formats.add(item["format"])
            for child in item.values():
                visit(child)
        elif isinstance(item, list):
            for child in item:
                visit(child)

    for document in value.documents:
        visit(document)
    missing = formats - set(module.FORMATS.checkers)
    require(not missing, "all-schema-format-plugins-required:" + ",".join(sorted(missing)))
    return value


class Http:
    def __init__(self, browser, event, report, oracle, deadline, failure_probe=None):
        self.endpoint = browser.target
        require(self.endpoint.scheme == "http" and self.endpoint.hostname in ("127.0.0.1", "::1")
                and not self.endpoint.username and not self.endpoint.password, "owned-gateway-required")
        self.origin, self.event = browser.origin, str(UUID(event))
        self.report, self.oracle, self.deadline = report, oracle, deadline
        self.failure_probe = failure_probe
        self.root = f"/api/concert/events/{self.event}"

    def probe_failed_delivery(self, operation):
        if self.failure_probe is None:
            return
        result = {"scope": "EVENT_OPERATION_AGGREGATE", "state": "UNAVAILABLE"}
        self.report["transport_failure"]["settlement_probe"] = result
        if self.deadline - time.time() < .75:
            result["state"] = "SKIPPED_WORK_DEADLINE"
            return
        try:
            counts = self.failure_probe(operation)
            require(type(counts) is dict and counts.keys() == FAILURE_COUNTS
                and all(type(value) is int and 0 <= value <= 1_000_000 for value in counts.values()),
                "closed-safe-failure-counts")
            result.update(state="CAPTURED", counts=counts)
        except Exception:
            # A diagnostic failure cannot mask the original delivery exception.
            # Never persist SQL, stderr, bindings or arbitrary exception text.
            pass

    def call(self, label, operation, actor, token, tail, *, payload=None, key=None, expected=(200,)):
        require(time.time() < self.deadline, "http-work-deadline")
        target = self.root + tail
        headers = {"Authorization": "Bearer " + token, "Origin": self.origin,
                   "X-MCR-Intent": "browser", "X-Correlation-Id": "own-report-" + label}
        raw = None
        if payload is not None:
            raw = json.dumps(payload, separators=(",", ":"), allow_nan=False).encode()
            require(len(raw) <= 65536, "report-request-bound")
            headers["Content-Type"] = "application/json"
        if key is not None:
            headers["Idempotency-Key"] = key
        connection = http.client.HTTPConnection(self.endpoint.hostname, self.endpoint.port,
                                                timeout=min(10, max(.01, self.deadline - time.time())))
        response = None
        failed_delivery = False
        try:
            connection.request("POST" if payload is not None else "GET", target, body=raw, headers=headers)
            response = connection.getresponse()
            body = response.read(131073)
            self.report["statuses"][label] = response.status
            require(response.status in expected, label + "-status")
            require(len(body) <= 131072 and response.getheader("Cache-Control") == "no-store"
                    and response.getheader("Content-Type", "").split(";", 1)[0] == "application/json"
                    and response.getheader("Location") is None
                    and response.getheader("ETag") is None, label + "-response-policy")
            value = launcher.json_object(body)
            path, _, query = target.partition("?")
            self.oracle.validate_http(value, operation, response.status, actor_id=actor,
                event_id=self.event, request_path=path, request_query=query, idempotency_key=key)
            return value
        except http.client.IncompleteRead as error:
            failed_delivery = True
            # Framing/counts identify a failed delivery boundary without retaining
            # partial private JSON, credentials, request keys or exception text.
            length = response.getheader("Content-Length") if response is not None else None
            transfer = response.getheader("Transfer-Encoding") if response is not None else None
            self.report["transport_failure"] = {
                "label": label, "operation_id": operation, "type": "IncompleteRead",
                "response_status": response.status if response is not None else None,
                "partial_bytes": len(error.partial),
                "expected_remaining_bytes": error.expected,
                "content_length": int(length) if isinstance(length, str) and len(length) <= 10
                    and length.isascii() and length.isdecimal() else None,
                "transfer_encoding": "ABSENT" if transfer is None else
                    "CHUNKED" if transfer.lower() == "chunked" else "OTHER",
            }
            raise
        finally:
            connection.close()
            if failed_delivery:
                self.probe_failed_delivery(operation)


def report_features(original, envs, concert, urls, signing_pairs):
    original(envs, concert, urls, signing_pairs)
    concert.update(CONCERT_REPORT_MODE="user", CONCERT_REPORT_CURSOR_KEY_HEX=secrets.token_hex(32),
        CONCERT_REPORT_RECEIPT_KEYS_JSON=json.dumps([
            {"id": "own_reports_ephemeral", "secret_hex": secrets.token_hex(32)}]))
    # Every managed feature secret remains independently generated in memory.
    keys = [v for k, v in concert.items() if k.endswith("_KEY_HEX")]
    keys += [entry["secret_hex"] for k, v in concert.items() if k.endswith("_KEYS_JSON")
             for entry in json.loads(v) if "secret_hex" in entry]
    require(len(keys) == len(set(keys)), "independent-report-managed-keys")
    launcher.validate_modes(envs, concert)


def effects(cluster, event):
    cluster.assert_owned()
    require(str(UUID(event)) == event, "event-sql-binding")
    tables = ("concert_reports", "concert_command_receipts", "concert_outbox", "concert_operations_audit")
    return tuple(int(cluster.sql("mcr_concert", f"select count(*) from {table} where event_id='{event}'::uuid"))
                 for table in tables)


def durable(cluster, event):
    """Only safe business columns are hashed; no bearer, credentials or full C03."""
    cluster.assert_owned()
    queries = {
        "reports": "select report_id,event_id,reporter_id,target_type,target_id,state,revision,created_at from concert_reports",
        "receipts": "select receipt_id,event_id,actor_id,operation_id,idempotency_key,state,resource_id,resource_revision,failure_reason,result_status,created_at,expires_at from concert_command_receipts",
        "outbox": "select outbox_id,event_id,receipt_id,resource_id,resource_revision from concert_outbox",
    }
    result = {}
    for name, query in queries.items():
        raw = cluster.sql("mcr_concert", f"select coalesce(jsonb_agg(to_jsonb(t) order by to_jsonb(t)::text),'[]'::jsonb)::text from ({query} where event_id='{event}'::uuid) t")
        result[name] = hashlib.sha256(raw.encode()).hexdigest()
    for database in ("mcr_api_gateway", "mcr_auth", "mcr_friend", "mcr_profile", "mcr_post", "mcr_notification", "mcr_concert"):
        raw = cluster.sql(database, "select coalesce(jsonb_agg(jsonb_build_array(version,encode(checksum,'hex'),success) order by version),'[]'::jsonb)::text from _sqlx_migrations")
        result[database + "_ledger"] = hashlib.sha256(raw.encode()).hexdigest()
    return result


def psql(cluster, database, app, query=None):
    cluster.assert_owned()
    args = [str(cluster.tools["psql"]), "-X", "--no-password", "-qAt", "-v", "ON_ERROR_STOP=1"]
    if query is not None:
        args += ["-c", query]
    return subprocess.Popen(args, env={**cluster.client_env(database), "PGAPPNAME": app}, cwd=cluster.root,
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)


def stop_session(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=3)
    for stream in (process.stdin, process.stdout, process.stderr):
        stream.close()


def settlement_failure_counts(cluster, event, operation, deadline):
    """One bounded read after socket close; counts do not identify the failed request."""
    require(str(UUID(event)) == event and operation in REPORT_OPERATIONS, "failure-probe-binding")
    require(deadline - time.time() >= .75, "failure-probe-work-deadline")
    query = f"""BEGIN READ ONLY;
SET LOCAL statement_timeout='250ms'; SET LOCAL lock_timeout='100ms';
WITH scopes AS MATERIALIZED (
 SELECT instance_id,scope_id,phase,serialized_at,delivery_completed_at,
        response_abandoned_at,cancellation_settled_at
 FROM public.concert_product_owner_scopes
 WHERE binding->>'event_id'='{event}' AND binding->>'operation'='{operation}'
), native AS (
 SELECT t.confirmed_at FROM public.concert_product_owner_transactions t
 JOIN scopes s USING(instance_id,scope_id)
), children AS (
 SELECT t.completed_at FROM public.concert_product_child_intents t
 JOIN scopes s USING(instance_id,scope_id)
)
SELECT json_build_object(
 'scopes',count(*), 'active',count(*) FILTER(WHERE phase='ACTIVE'),
 'sealed',count(*) FILTER(WHERE phase='SEALED'),
 'root_finished',count(*) FILTER(WHERE phase='ROOT_FINISHED'),
 'serialized',count(*) FILTER(WHERE serialized_at IS NOT NULL),
 'delivery_completed',count(*) FILTER(WHERE delivery_completed_at IS NOT NULL),
 'response_abandoned',count(*) FILTER(WHERE response_abandoned_at IS NOT NULL),
 'cancellation_settled',count(*) FILTER(WHERE cancellation_settled_at IS NOT NULL),
 'registered_native',(SELECT count(*) FROM native),
 'confirmed_native',(SELECT count(*) FROM native WHERE confirmed_at IS NOT NULL),
 'operations_witnesses',(SELECT count(*) FROM public.concert_operations_native_witnesses w
                         JOIN scopes s USING(instance_id,scope_id)),
 'child_intents',(SELECT count(*) FROM children),
 'pending_children',(SELECT count(*) FROM children WHERE completed_at IS NULL)) FROM scopes;
ROLLBACK;"""
    process = psql(cluster, "mcr_concert", "own_reports_failure_probe", query)
    try:
        output, _ = process.communicate(timeout=min(.5, max(.01, deadline - time.time())))
        require(process.returncode == 0 and len(output) <= 4096, "failure-probe-query")
        return launcher.json_object(output.encode())
    finally:
        # No grace period for a diagnostic child after its finite read budget.
        if process.poll() is None:
            process.kill()
        stop_session(process)


def wait_sql(cluster, predicate, label, deadline):
    while time.time() < deadline:
        if cluster.sql("postgres", predicate) == "t":
            return
        time.sleep(.005)
    require(False, label)


def revocation(http, cluster, actor, token, report):
    """Hold only a table lock to pause the actual read after canonical admission.

    The Auth status writer must block behind that HTTP read, then commit after
    the original HTTP terminal. No test owner grant or positive identity SQL.
    """
    holder = psql(cluster, "mcr_concert", "own_reports_table_hold")
    writer = None
    try:
        holder.stdin.write("BEGIN; LOCK TABLE concert_reports IN ACCESS EXCLUSIVE MODE; SELECT 'READY';\n")
        holder.stdin.flush()
        require(bool(select.select([holder.stdout], [], [], 3)[0]) and holder.stdout.readline().strip() == "READY",
                "owned-table-hold-ready")
        with ThreadPoolExecutor(max_workers=1) as pool:
            read = pool.submit(http.call, "held-own-read", "listOwnConcertReports", actor, token, "/me/reports")
            try:
                end = min(http.deadline, time.time() + .6)
                wait_sql(cluster, "select exists(select 1 from pg_stat_activity where datname='mcr_concert' and wait_event_type='Lock' and query like '%from concert_reports%' and application_name<>'own_reports_table_hold')",
                         "actual-http-own-read-did-not-wait", end)
                app = "own_reports_auth_writer_" + secrets.token_hex(6)
                writer = psql(cluster, "mcr_auth", app,
                    f"set lock_timeout='2s'; update users set status='SUSPENDED' where id='{str(UUID(actor))}'::uuid")
                wait_sql(cluster, f"select exists(select 1 from pg_stat_activity where application_name='{app}' and cardinality(pg_blocking_pids(pid))>0)",
                         "auth-writer-not-excluded-by-http-read", end)
                require(writer.poll() is None and not read.done(), "actual-read-writer-ordering")
                report["auth_writer_excluded_during_actual_http"] = True
            finally:
                holder.stdin.write("COMMIT;\n")
                holder.stdin.flush()
            read.result(timeout=10)
        require(writer is not None and writer.wait(timeout=3) == 0, "auth-withdrawal-commit")
        http.call("withdrawn-own-read", "listOwnConcertReports", actor, token, "/me/reports", expected=(401, 403))
        report["withdrawal_denies_next_http"] = True
    finally:
        stop_session(holder)
        if writer is not None:
            stop_session(writer)


def journey(stack, browser, principals, event, report, deadline):
    require(len(principals) == 3 and len({p[0] for p in principals}) == 3, "three-real-principals")
    # Browser.target is the real Gateway; origin is the allowed frontend origin.
    http = Http(browser, event, report, contract(), deadline,
        failure_probe=lambda operation: settlement_failure_counts(stack.cluster, event, operation, deadline))
    a, b, c = principals
    baseline = effects(stack.cluster, event)
    require(baseline == (0, 0, 0, 0), "fresh-report-business-state")
    commands = []
    for label, owner, target in (("a-report-b", a, b), ("a-report-c", a, c), ("b-report-c", b, c)):
        key = str(uuid4())
        body = {"target_type": "USER", "target_id": target[0], "reason": "HARASSMENT",
                "text": "private reporter canary " + secrets.token_hex(8), "evidence_media_ids": []}
        value = http.call(label, "createConcertReport", owner[0], owner[2], "/reports", payload=body, key=key)
        require(value["target_id"] == target[0] and value["state"] == "OPEN", "actual-safe-report-resource")
        commands.append((owner, key, body, value))
    require(effects(stack.cluster, event) == (3, 3, 3, 3), "one-effect-per-public-command")
    owner, key, body, original = commands[0]
    replay = http.call("exact-replay", "createConcertReport", owner[0], owner[2], "/reports", payload=body, key=key)
    require(replay == original, "same-original-public-report-replay")
    error = http.call("changed-body", "createConcertReport", a[0], a[2], "/reports",
                     payload={**body, "text": "changed"}, key=key, expected=(409,))
    require(error["details"]["reason"] == "IDEMPOTENCY_CONFLICT", "changed-body-key-conflict")
    status_tail = "/commands?operation_id=createConcertReport&idempotency_key=" + key
    receipt = http.call("original-command-status", "getConcertCommandStatus", a[0], a[2], status_tail)
    require(receipt["state"] == "SUCCEEDED" and receipt["result_status"] == 200
            and receipt["resource_id"] == original["report_id"], "actual-original-receipt-resource")
    http.call("foreign-command-status", "getConcertCommandStatus", b[0], b[2], status_tail, expected=(404,))
    mine = {command[3]["report_id"] for command in commands[:2]}
    first = http.call("own-page-one", "listOwnConcertReports", a[0], a[2], "/me/reports?limit=1&state=OPEN")
    require(len(first["items"]) == 1 and first["next_cursor"] is not None, "actual-pagination-first")
    cursor = first["next_cursor"]
    tail = "/me/reports?limit=1&state=OPEN&cursor=" + cursor
    second = http.call("own-page-two", "listOwnConcertReports", a[0], a[2], tail)
    require(len(second["items"]) == 1 and {first["items"][0]["report_id"], second["items"][0]["report_id"]} == mine,
            "actual-private-page-membership")
    other = http.call("other-own-page", "listOwnConcertReports", b[0], b[2], "/me/reports")
    require({r["report_id"] for r in other["items"]} == {commands[2][3]["report_id"]}, "caller-owned-only")
    http.call("cross-actor-cursor", "listOwnConcertReports", b[0], b[2], tail, expected=(400,))
    http.call("changed-limit-cursor", "listOwnConcertReports", a[0], a[2], tail.replace("limit=1", "limit=2"), expected=(400,))
    http.call("changed-state-cursor", "listOwnConcertReports", a[0], a[2], tail.replace("state=OPEN", "state=RESOLVED"), expected=(400,))
    http.call("foreign-self-selector", "listOwnConcertReports", a[0], a[2], "/me/reports?reporter_id=" + b[0], expected=(400,))
    require(effects(stack.cluster, event) == (3, 3, 3, 3), "reads-replays-conflicts-have-no-command-effects")
    require(stack.cluster.sql("postgres", "select count(*) from pg_prepared_xacts") == "0", "actual-terminal-settlement")
    before, old_pid = durable(stack.cluster, event), stack.cluster.assert_postmaster()
    stack.cluster.stop()
    stack.cluster.start()
    require(stack.cluster.assert_postmaster() != old_pid and durable(stack.cluster, event) == before,
            "actual-restart-business-and-ledger-durability")
    report["restart_business_receipt_outbox_ledgers"] = before
    require(http.call("restart-command-status", "getConcertCommandStatus", a[0], a[2], status_tail) == receipt,
            "restart-original-public-receipt")
    require(http.call("restart-exact-replay", "createConcertReport", a[0], a[2], "/reports", payload=body, key=key) == original,
            "restart-original-public-resource")
    revocation(http, stack.cluster, a[0], a[2], report)
    require(effects(stack.cluster, event) == (3, 3, 3, 3), "read-withdrawal-no-command-effects")
    require(stack.cluster.sql("postgres", "select count(*) from pg_prepared_xacts") == "0", "final-prepared-zero")
    report.update(journey="PASS", read_command_effects="NONE", final_prepared=0)


def run(args):
    result = {"scope": "BE36 public USER/no-media Report and own status", "result": "FAIL", "statuses": {},
              "journey": "NOT_RUN", "full_c03_retained_or_hashed": False,
              "moderator_media_evidence_review": "UNAVAILABLE", "whole_be36_be38": "OPEN"}
    original_auth, original_features = launcher.auth_provenance, launcher.managed_features
    original_cleanup = launcher.cleanup_owned
    stack_args = SimpleNamespace(**vars(args))
    stack_args.report = args.report.with_suffix(".stack.json")
    # Keep producer metadata live during the synchronous journey; stop the real
    # control owner immediately afterward instead of holding an external slot.
    stack_args.hold_seconds = max(60, args.hold_seconds)
    world, _, dev = launcher.load_helpers(ROOT)
    original_projection, original_control = world.retain_public_projection, dev.Control
    context, owned_processes, owned_roots = {}, [], []
    original_cluster, original_child = dev.migration.Cluster, dev.runtime.Child
    custody = args.report.with_suffix(".owner.json")

    def owner_receipt():
        value = {"roots": [{"root": str(c.root), "identity": list(c.identity), "token": c.token}
                           for c in owned_roots], "service_pids": owned_processes}
        fd = os.open(custody, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, "w") as stream:
            json.dump(value, stream)

    def cluster(*values, **kwargs):
        owned = original_cluster(*values, **kwargs)
        owned_roots.append(owned)
        owner_receipt()  # Before initdb/postmaster: the independent guardian knows custody.
        return owned

    def child(*values, **kwargs):
        owned = original_child(*values, **kwargs)
        owned_processes.append(owned.process.pid)
        owner_receipt()
        return owned

    def authenticate(stack, browser, original_report, principals, event, *rest):
        provenance = original_auth(stack, browser, original_report, principals, event, *rest)
        context.update(stack=stack, browser=browser, principals=principals, event=event)
        return provenance

    def project(*values, **kwargs):
        projection = original_projection(*values, **kwargs)
        journey(**context, report=result, deadline=args.allocation_work_deadline_epoch)
        return projection

    def control(stack):
        owned = original_control(stack)
        owned.stopping = True
        return owned

    def cleanup(stack, owned_control, guard):
        try:
            if stack.cluster is not None and stack.cluster.started:
                require(time.monotonic() < guard.hard_deadline, "diagnostic-cleanup-deadline")
                stack.cluster.assert_owned()
                # Counts, not GIDs or private Root/child capabilities. Observe
                # before teardown while custody is still owned and verifiable.
                counts = launcher.json_object(stack.cluster.sql("postgres",
                    "select coalesce(json_object_agg(database,n),'{}'::json)::text from "
                    "(select database,count(*) n from pg_prepared_xacts group by database) p").encode())
                require(all(name in {"mcr_auth", "mcr_friend", "mcr_post", "mcr_profile",
                                     "mcr_notification", "mcr_concert", "mcr_api_gateway"}
                            and isinstance(count, int) and count >= 0 for name, count in counts.items()),
                        "prepared-database-counts")
                result["prepared_before_cleanup_by_database"] = counts
        except Exception:
            result["prepared_before_cleanup_by_database"] = "UNAVAILABLE"
        # Diagnostic failure never skips or replaces the original cleanup.
        return original_cleanup(stack, owned_control, guard)

    code = 1
    try:
        with patch.object(launcher, "auth_provenance", authenticate), patch.object(launcher, "managed_features",
                lambda *values: report_features(original_features, *values)), \
                patch.object(world, "retain_public_projection", project), patch.object(dev, "Control", control), \
                patch.object(dev.migration, "Cluster", cluster), patch.object(dev.runtime, "Child", child), \
                patch.object(launcher, "cleanup_owned", cleanup):
            code = STACK_RUN(stack_args)
        stack_report = launcher.json_object(launcher.private_input(stack_args.report))
        result["stack"] = {k: stack_report.get(k) for k in
                           ("result", "failure", "failure_type", "cleanup", "prepared_before_cleanup", "process_stops", "resource_bounds", "source_revision", "cohort")}
        require(code == 0 and result["journey"] == "PASS" and stack_report["cleanup"] == "PASS"
                and stack_report.get("prepared_before_cleanup") == "0", "actual-stack-and-journey-cleanup")
        require(all(not c.root.exists() for c in owned_roots), "exact-owned-stack-roots-absent")
        for pid in owned_processes:
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                continue
            require(False, "owned-service-process-still-alive")
        result["owned_roots_absent"] = True
        result["owned_service_processes_absent"] = True
        result["result"] = "PASS"
    except Exception as error:
        result.update(failure=error.args[0] if isinstance(error, launcher.InputRefusal) else "report-http-operation-failed",
                      failure_type=type(error).__name__)
    finally:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "w") as stream:
            json.dump(result, stream, indent=2)
            stream.write("\n")
    return int(result["result"] != "PASS")


def main(argv=None):
    # Preserve the launcher's CLI bounds/refusal path; only its selected run is replaced.
    original = launcher.run
    try:
        launcher.run = run
        return launcher.main(argv)
    finally:
        launcher.run = original


if __name__ == "__main__":
    raise SystemExit(main())
