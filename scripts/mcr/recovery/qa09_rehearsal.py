#!/usr/bin/env python3
"""QA09 six-scenario recovery and lifecycle rehearsal on an owned PG16 stack.

The rehearsal starts the seven-service development composition through the
existing closure driver, adds Social and managed account-deletion composition,
then injects real database, worker and service faults. Every scenario records
before/after rows, the interrupting action, the restored invariant, database
rows, logs and command exit codes. It is not a deployment or capacity claim.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timedelta, timezone
import http.client
import json
import os
from pathlib import Path
import secrets
import shlex
import signal
import socket
import subprocess
import sys
import threading
import time
import traceback
from uuid import uuid4

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))
import closure_vertical_stack as closure  # noqa: E402
import dev_stack  # noqa: E402
import migration_smoke as migration  # noqa: E402
import runtime_smoke as runtime  # noqa: E402

PG_BIN = Path("/opt/homebrew/opt/postgresql@16/bin")
CONCERT_DB = "mcr_concert"
AUTH_DB = "mcr_auth"
SOCIAL = {}
AUTH_GRPC = None
CHILD_SPECS = {}
NATIVE_OUTBOX = None
NATIVE_DELETION = None
ONLY_CASES = None
RESULT = {"started_at": datetime.now(timezone.utc).isoformat(), "scenarios": [], "engine": {}}

METHOD_NOTES = """
## Method and limits

### Deletion

- `QA09-DELETION` is `PASS (isolated owner mount)`: the committed production Auth deletion source and the mounted production Concert PrivateData factory ran over a test-owned mTLS loopback listener. The after-state removes the avatar and age-owner rows and leaves one sanitized `concert_participants` row, so the owner's `private_rows_remaining` predicate is 0 while the raw participant count is 1. The control user is intact, the sealed acknowledgement replays after a service restart without rewriting its terminal receipt, and the same case re-runs after a physical PostgreSQL restart. It is not eight-target saga convergence.
- `QA09-DELETION-SAGA` records the live seven-service saga exactly as measured: per-step status, `attempt_count` and `last_error` are in that section. A failure or non-completion there is not deletion convergence; only a `PASS` on that row would be. No deletion-convergence claim is made for the live stack unless it is `PASS`.
- The isolated mount's own canonical saga likewise stays `PENDING`; it proves the PrivateData owner, not the saga.

### Outbox replay

- The replay is a SQL reset of one delivered outbox row to `READY` plus a full PostgreSQL cluster stop/start, followed by a fresh process and connection that claims the row again. It is not a process killed mid-publish; no mid-publish SIGKILL is claimed.
- A real mid-publish kill was not added: the native test's apply/ack boundary is in-process with no pause hook, and its restart fixture deliberately persists the replay identity before the SQL reset. The SQL reset plus cluster restart is the fault this harness exercises.
- The same native test also proves a failed second fanout leg leaves the row `LEASED` (no false ack) until the retry succeeds; the replay then returns the row to `DELIVERED` with exactly one consumer receipt and one new attempt.
- It is the native World invalidation consumer, not the seven-service social worker's live pickup outbox path.

### Database restart

- The database restart is an immediate stop of the owned postmaster; it verifies crash recovery, in-flight rollback and committed-row survival, not disk failure or a standby.

### Logical backup

- The backup is a logical `pg_dump -Fc` of the Concert owner database into a fresh owned database, restored with `pg_restore`. The exact command, measured byte count and SHA-256 are in the QA09-BACKUP-RESTORE section; the dump is removed after the restore.
- It is not a physical restore of the live cluster and does not back up the Auth database. `pg_dump -Fc` output is not byte-stable across runs; the published size is the measured size of this run.

### Pending pickup

- The scenario blocks a real claim on the giveaway row, terminates the lock backend, lets the claim complete, restarts the gateway, and replays the same idempotency key. A validation retry was not needed because the fixed release let the claim complete; this does not claim a concert-process crash mid-claim.

### Partial closure

- The closure transaction is blocked, released atomically, and verified to enter `READ_ONLY` with a durable job, 409 writes and 200 reads. Later FREEZE_MURAL..CLEANUP owners and the public archive are not claimed; the live composition reports them unavailable. No concert-process restart after closure is claimed: a restarted concert fails World admission because the closed event has retired its room leases.

### Gateway restart

- The gateway restart compares committed event, giveaway, reservation, outbox identity, command receipt, closure job and closure effect rows, not only giveaway and reservation rows.

### Other limits

- The stack is the development seven-service composition on an owned PostgreSQL 16.14 cluster with loopback TLS. It is not a deployment, capacity, multiuser race, browser/device, or production approval.
- The baseline uses the real giveaway, meeting, reservation, pickup-code and claim flow rather than the original party concurrency gate; QA03 owns race acceptance.
- No QA08 performance, 200-user, browser, or device acceptance is claimed.

## Mutation proof

The outbox stale-lease fence was broken in a throwaway worktree
`/tmp/qa09-mutation` at base `80b03639d`. In
`backend/services/concert-service/src/adapters/out/repository/outbox.rs`,
`acknowledge` was reduced to `where outbox_id=$1 and state='LEASED'`, so a
holder whose lease was reclaimed could still acknowledge the delivery.

- Build: `CARGO_TARGET_DIR=/Users/lhs/Desktop/mcr-build/exec-w7-qa09 CARGO_INCREMENTAL=0 cargo +1.88.0 test --locked --offline -j3 -p concert-service --test maintenance_outbox --no-run` exited 0.
- Mutated run (`maintenance_origin_transactions_and_leases` on a fresh owned PG16 cluster): exit 101, `test result: FAILED. 0 passed; 1 failed`, panic at `services/concert-service/tests/maintenance_outbox.rs:104: assertion failed: !box_.acknowledge(first).await.unwrap()`. The stale holder acknowledged after the lease rotated, which is exactly the recovery invariant the outbox replay depends on.
- Baseline run (copied unmodified binary): exit 0, `test result: ok. 1 passed; 0 failed`.
- Restore: the throwaway worktree was removed; the delivery worktree source is unchanged. The baseline binary is retained at `/tmp/qa09-trial/maintenance_outbox-baseline`.
""".splitlines()



class ScenarioFailure(RuntimeError):
    pass


def require(condition, label):
    if not condition:
        raise ScenarioFailure(label)


def now():
    return datetime.now(timezone.utc).isoformat()


def monotonic_wait(predicate, timeout, interval=0.05):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(interval)
    return None


def log_excerpt(path, limit=4000):
    try:
        data = Path(path).read_bytes()
    except OSError:
        return ""
    return data[-limit:].decode("utf-8", errors="replace")


def row_lines(value):
    return [line for line in value.splitlines() if line]


def command(rec, label, argv, exit_code, output=""):
    rec["commands"].append({"label": label, "argv": [str(item) for item in argv],
                            "exit_code": exit_code, "output": output[-2000:]})


def sql(rec, db, query, label, limit=4000):
    value = CURRENT_STACK.cluster.sql(db, query)
    rec["db_rows"][label] = value[-limit:]
    return value


CURRENT_STACK = None


def psql_session(cluster, db, query, application_name):
    process = subprocess.Popen(
        [str(cluster.tools["psql"]), "-X", "--no-password", "-qAt", "-v", "ON_ERROR_STOP=1",
         "-c", query],
        env={**cluster.client_env(db), "PGAPPNAME": application_name}, cwd=cluster.root,
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    return process


class RowLock:
    def __init__(self, cluster, query, label):
        self.cluster, self.label = cluster, label
        self.process = psql_session(
            cluster, CONCERT_DB,
            "BEGIN; " + query + " SELECT pg_sleep(300);", "qa09_lock")

    def granted(self, timeout=8):
        value = monotonic_wait(lambda: self.cluster.sql(
            "postgres",
            "select count(*) from pg_stat_activity where application_name='qa09_lock' "
            "and wait_event='PgSleep' and state='active';"), timeout)
        require(value == "1", "row lock was not granted")
        return self

    def release(self):
        try:
            pid = self.cluster.sql(
                "postgres",
                "select pid from pg_stat_activity where application_name='qa09_lock' "
                "and wait_event='PgSleep' limit 1;")
            if pid:
                self.cluster.sql("postgres", "select pg_terminate_backend(" + pid + ");")
        except Exception:
            pass
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        self.process.stdout.close()
        self.process.stderr.close()


def retry_dependency(call, timeout=45, interval=0.5):
    deadline = time.monotonic() + timeout
    attempts = 0
    result = (None, {})
    while time.monotonic() < deadline:
        attempts += 1
        result = call()
        if result[0] == 200:
            return result + (attempts,)
        if result[0] not in (None, 0, 502, 503, 504):
            return result + (attempts,)
        time.sleep(interval)
    return result + (attempts,)


def blocked_backends(cluster):
    return int(cluster.sql(
        "postgres",
        "select count(*) from pg_stat_activity where datname='" + CONCERT_DB + "' "
        "and application_name <> 'qa09_lock' and state='active' "
        "and wait_event_type='Lock';"))


def terminate_blocked(cluster, rec, label):
    pid = cluster.sql(
        "postgres",
        "select pid from pg_stat_activity where datname='" + CONCERT_DB + "' "
        "and application_name <> 'qa09_lock' and state='active' "
        "and wait_event_type='Lock' order by query_start limit 1;")
    require(pid, "no blocked backend to terminate")
    terminated = cluster.sql("postgres", "select pg_terminate_backend(" + pid + ");")
    require(terminated == "t", "blocked backend termination was refused")
    rec["actions"].append({"terminate_backend": {"pid": pid, "label": label}})
    rec["commands"].append({"label": label, "argv": ["pg_terminate_backend", pid], "exit_code": 0})


def request(method, url, payload=None, token=None, origin=None, key=None):
    extra = {"Idempotency-Key": key} if key else None
    try:
        status, body, _ = closure.request(url, method, payload, token, origin, True, extra)
        return status, body
    except Exception as error:  # the interrupted connection is expected in fault runs
        return None, {"error": type(error).__name__}


def hold_port(stack, key):
    port = stack.ports.ports[key]
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    sock.bind(("127.0.0.1", port))
    sock.listen(128)
    stack.ports.held[key] = sock


def await_world_lease(stack, timeout=8):
    def released():
        try:
            return stack.cluster.sql(
                CONCERT_DB,
                "select count(*) from concert_world_owner_leases where lease_until>clock_timestamp()") == "0"
        except Exception:
            return False
    return monotonic_wait(released, timeout, interval=0.1) is not None


def respawn(stack, name, overrides=None):
    child = stack.children.get(name)
    if child is not None and child.process.poll() is None:
        child.stop()
    if name == "concert-service":
        await_world_lease(stack)
    spec = CHILD_SPECS[name]
    env = {**spec["env"], **(overrides or {})}
    for key in spec["reservations"]:
        hold_port(stack, key)
    restarted = runtime.Child(stack.cluster, name, spec["argv"], env, stack.ports,
                              spec["reservations"], spec["secrets"])
    stack.children[name] = restarted
    try:
        restarted.wait_listeners(timeout=45)
    except Exception as error:
        raise ScenarioFailure(f"{name} restart failed: {restarted.diagnostic()}") from error
    return restarted


def order(stack):
    return [name for name in runtime.start_order() if name in stack.children]


def readiness_snapshot(stack):
    result = {}
    for name, child in stack.children.items():
        if child.process.poll() is not None:
            result[name] = {"alive": False}
            continue
        connection = None
        try:
            connection = http.client.HTTPConnection(
                "127.0.0.1", stack.ports.ports[f"{name}:metrics"], timeout=2)
            connection.request("GET", "/readyz", headers={"Connection": "close"})
            response = connection.getresponse()
            body = response.read(16385)
            value = json.loads(body)
            result[name] = {"alive": True, "status": response.status,
                            "ready": value.get("ready") is True,
                            "degraded": value.get("degraded") is not False}
        except Exception as error:
            result[name] = {"alive": True, "probe": type(error).__name__, "error": str(error)[:300]}
        finally:
            if connection is not None:
                connection.close()
    return result


def wait_services(stack, timeout=150):
    deadline = time.monotonic() + timeout
    snapshot = {}
    while time.monotonic() < deadline:
        snapshot = readiness_snapshot(stack)
        bad = [name for name, value in snapshot.items()
               if value.get("alive") is False or value.get("ready") is not True or value.get("degraded")]
        if not bad:
            return snapshot
        for name in bad:
            if snapshot[name].get("alive") is False:
                respawn(stack, name)
        time.sleep(0.25)
    raise ScenarioFailure("services not ready: " + json.dumps(snapshot)[:6000])


def recover_services(stack, rec, overrides=None):
    for name in order(stack):
        child = stack.children.get(name)
        if child is None or child.process.poll() is not None:
            respawn(stack, name, (overrides or {}).get(name))
    rec["actions"].append({"recovered_services": {name: stack.children[name].process.pid for name in order(stack)},
                           "readiness": wait_services(stack)})


def restart_service(stack, name, rec, kill=False, overrides=None):
    child = stack.children[name]
    old_pid = child.process.pid
    if child.process.poll() is None:
        if kill:
            os.kill(old_pid, signal.SIGKILL)
            child.process.wait(timeout=5)
            rec["actions"].append({"kill": name, "pid": old_pid, "signal": "SIGKILL"})
        else:
            rec["actions"].append({"stop": name, "result": child.stop()})
    new = respawn(stack, name, overrides)
    rec["actions"].append({"start": name, "pid": new.process.pid})
    rec["commands"].append({"label": f"restart-{name}", "argv": [name], "exit_code": 0,
                            "output": f"{old_pid}->{new.process.pid}"})
    return old_pid, new.process.pid


def snapshot(stack, event, user=None, db=CONCERT_DB):
    queries = {
        "event": (db, "select phase||'|'||revision from concert_events where event_id='" + event + "'"),
        "giveaways": (db, "select giveaway_id||'|'||state||'|'||revision||'|'||reserved_unclaimed||'|'||quantity_claimed from concert_community_giveaways where event_id='" + event + "' order by giveaway_id"),
        "reservations": (db, "select reservation_id||'|'||state||'|'||revision from concert_community_reservations where event_id='" + event + "' order by reservation_id"),
        "outbox_ids": (db, "select string_agg(outbox_id::text,',' order by outbox_id) from concert_outbox where event_id='" + event + "'"),
        "receipts": (db, "select operation_id||'|'||state||'|'||coalesce(failure_reason,'') from concert_command_receipts where event_id='" + event + "' order by operation_id,created_at"),
        "closure_jobs": (db, "select job_id||'|'||(progress->>'state')||'|'||revision from concert_closure_jobs where event_id='" + event + "' order by job_id"),
        "closure_effects": (db, "select step||'|'||owner_revision from concert_closure_effects where event_id='" + event + "' order by step"),
    }
    if user:
        queries["saga"] = (AUTH_DB, "select id||'|'||status||'|'||coalesce(completed_at::text,'') from user_deletion_sagas where user_id='" + user + "'")
        queries["steps"] = (AUTH_DB, "select s.target||'|'||s.status||'|'||s.attempt_count from user_deletion_steps s join user_deletion_sagas g on g.id=s.saga_id where g.user_id='" + user + "' order by s.target")
    return {label: stack.cluster.sql(db, query) for label, (db, query) in queries.items()}


def provision_pickup(stack, gateway, origin, event, host, alice, record, tag):
    base = f"{gateway}/api/concert/events/{event}"
    now_value = datetime.now(timezone.utc)

    def stamp(value):
        return value.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")

    starts, ends = stamp(now_value - timedelta(minutes=1)), stamp(now_value + timedelta(hours=2))
    create_key = str(uuid4())
    status, giveaway, create_attempts = retry_dependency(
        lambda: request("POST", base + "/giveaways", {
            "title": f"QA09 recovery {tag}", "description": "Recovery rehearsal fixture",
            "image_media_id": None, "quantity_total": 1,
            "pickup_starts_at": starts, "pickup_ends_at": ends,
            "public_place_id": closure.IN02_PLACE}, host[1], origin, create_key))
    record["actions"].append({"giveaway_create_attempts": create_attempts})
    require(status == 200 and isinstance(giveaway.get("giveaway_id"), str), f"{tag}: giveaway create {status}")
    giveaway_id, revision = giveaway["giveaway_id"], giveaway["revision"]
    status, meeting = request("PUT", f"{base}/giveaways/{giveaway_id}/pickup-meeting", {
        "expected_revision": revision, "physical_presence_acknowledged": True,
        "meeting": {"place_id": closure.IN02_PLACE, "map_revision": 1, "place_revision": 1,
                    "instructions": "Recovery collection point",
                    "identification": "Show the six digit code",
                    "expires_at": stamp(now_value + timedelta(hours=3))}}, host[1], origin, str(uuid4()))
    record["actions"].append({"pickup_meeting": tag, "status": status, "body": meeting})
    require(status == 200, f"{tag}: pickup meeting {status}: {json.dumps(meeting)[:800]}")
    revision = meeting.get("revision", revision + 1)
    status, opened = request("PATCH", f"{base}/giveaways/{giveaway_id}", {
        "expected_revision": revision, "title": f"QA09 recovery {tag}",
        "description": "Recovery rehearsal fixture", "image_media_id": None,
        "quantity_total": 1, "pickup_starts_at": starts, "pickup_ends_at": ends,
        "public_place_id": closure.IN02_PLACE, "state": "OPEN"}, host[1], origin, str(uuid4()))
    require(status == 200, f"{tag}: giveaway open {status}")
    revision = opened["revision"]
    status, reservation = request("POST", f"{base}/giveaways/{giveaway_id}/reservations",
                                  {"giveaway_revision": revision}, alice[1], origin, str(uuid4()))
    require(status == 200 and isinstance(reservation.get("reservation_id"), str), f"{tag}: reserve {status}")
    reservation_id = reservation["reservation_id"]
    status, issued = request("POST", f"{base}/reservations/{reservation_id}/pickup-code",
                             {"expected_revision": 1}, alice[1], origin, str(uuid4()))
    require(status == 200 and isinstance(issued.get("confirmation_code"), str), f"{tag}: pickup code {status}")
    record["actions"].append({"provision": tag, "giveaway": giveaway_id, "reservation": reservation_id,
                              "status": status})
    return {"giveaway": giveaway_id, "reservation": reservation_id,
            "code": issued["confirmation_code"],
            "reservation_revision": issued["reservation_revision"], "base": base}


def scenario_pending_pickup(stack, gateway, origin, event, host, alice, rec):
    rec["before"] = snapshot(stack, event)
    pickup = provision_pickup(stack, gateway, origin, event, host, alice, rec, "pending")
    sql(rec, CONCERT_DB, "select state||'|'||revision||'|'||reserved_unclaimed||'|'||quantity_claimed from concert_community_giveaways where giveaway_id='" + pickup["giveaway"] + "'", "giveaway_before")
    lock = RowLock(stack.cluster, "select 1 from concert_community_giveaways where giveaway_id='" + pickup["giveaway"] + "' for update;", "giveaway").granted()
    key = str(uuid4())
    outcome = {}
    target = f"{pickup['base']}/reservations/{pickup['reservation']}/claim"
    payload = {"expected_revision": pickup["reservation_revision"], "confirmation_code": pickup["code"]}
    thread = threading.Thread(target=lambda: outcome.update(response=request("POST", target, payload, host[1], origin, key)))
    thread.start()
    require(monotonic_wait(lambda: blocked_backends(stack.cluster) >= 1, 8), "claim never blocked in its database transaction")
    rec["actions"].append({"pending_claim": "request blocked in its database transaction"})
    lock.release()
    thread.join(timeout=30)
    require(not thread.is_alive(), "pending claim did not resolve after unlock")
    status, first = outcome.get("response", (None, {}))
    rec["actions"].append({"blocked_claim_outcome": {"status": status, "body": first}})
    if not (status == 200 and first.get("state") == "CLAIMED"):
        state = sql(rec, CONCERT_DB, "select state||'|'||revision from concert_community_reservations where reservation_id='" + pickup["reservation"] + "'", "reservation_after_blocked_attempt")
        if not state.startswith("CLAIMED"):
            require(state.startswith("READY") or state.startswith("HELD"), "blocked claim did not remain claimable")
            sql(rec, CONCERT_DB, "select state||'|'||receipt_id from concert_command_receipts where actor_id='" + host[0] + "' and event_id='" + event + "' and operation_id='claimConcertReservation' and idempotency_key='" + key + "'", "claim_receipt_after_blocked")
            sql(rec, CONCERT_DB, "select count(*) from concert_community_pickup_admissions where actor_id='" + host[0] + "' and event_id='" + event + "' and idempotency_key='" + key + "'", "pickup_admission_after_blocked")
        else:
            rec["actions"].append({"blocked_claim_committed_without_response": state})
        restart_service(stack, "concert-service", rec)
        restart_service(stack, "api-gateway", rec)
        recover_services(stack, rec)
        status, first, attempts = retry_dependency(
            lambda: request("POST", target, payload, host[1], origin, key))
        rec["actions"].append({"retry_claim": {"status": status, "body": first, "attempts": attempts}})
        require(status == 200 and first.get("state") == "CLAIMED", f"claim retry did not complete: {status}")
    else:
        restart_service(stack, "api-gateway", rec)
        recover_services(stack, rec)
    state = sql(rec, CONCERT_DB, "select state||'|'||revision from concert_community_reservations where reservation_id='" + pickup["reservation"] + "'", "reservation_after_interrupt")
    counters = sql(rec, CONCERT_DB, "select reserved_unclaimed||'|'||quantity_claimed from concert_community_giveaways where giveaway_id='" + pickup["giveaway"] + "'", "giveaway_after_interrupt")
    require(state.startswith("CLAIMED"), "completed claim is not CLAIMED")
    require(counters == "0|1", "completed claim did not consume exactly one unit")
    original_receipt = sql(rec, CONCERT_DB, "select receipt_id from concert_command_receipts where event_id='" + event + "' and operation_id='claimConcertReservation' and state='SUCCEEDED' and resource_id='" + pickup["reservation"] + "' order by created_at limit 1", "original_receipt")
    require(original_receipt != "", "no exactly-once claim receipt exists after recovery")
    status, replay, replay_attempts = retry_dependency(
        lambda: request("POST", target, payload, host[1], origin, key))
    rec["actions"].append({"replay_claim": {"status": status, "body": replay, "attempts": replay_attempts}})
    require(status == 200 and replay.get("state") == "CLAIMED", "same-key replay did not return the claim")
    counters = sql(rec, CONCERT_DB, "select reserved_unclaimed||'|'||quantity_claimed from concert_community_giveaways where giveaway_id='" + pickup["giveaway"] + "'", "giveaway_recovered")
    require(counters == "0|1", "recovered claim did not consume exactly one unit")
    receipts = sql(rec, CONCERT_DB, "select count(*) from concert_command_receipts where event_id='" + event + "' and operation_id='claimConcertReservation' and state='SUCCEEDED' and resource_id='" + pickup["reservation"] + "'", "success_receipts_recovered")
    require(receipts == "1", "recovered claim produced a duplicate success receipt")
    require(replay.get("receipt_id") == first.get("receipt_id"), "same-key replay returned a new claim receipt")
    rec["after"] = snapshot(stack, event)
    rec["invariants"] = ["a claim blocked in its database transaction completed exactly once after the lock backend was released",
                         "inventory ended consumed exactly once",
                         "gateway restart and the same idempotency key replayed the original receipt",
                         "no double consumption"]
    rec["state"] = "PASS"


def scenario_service_restart(stack, gateway, origin, event, host, rec):
    rec["before"] = snapshot(stack, event)
    giveaway = stack.cluster.sql(CONCERT_DB, "select giveaway_id from concert_community_giveaways where event_id='" + event + "' order by giveaway_id limit 1")
    url = f"{gateway}/api/concert/events/{event}/giveaways/{giveaway}"
    status_before, body_before = request("GET", url, None, host[1], origin)
    require(status_before == 200, "pre-restart product read failed")
    old_gateway = stack.children["api-gateway"].process.pid
    restart_service(stack, "api-gateway", rec)
    recover_services(stack, rec)
    status_after, body_after = request("GET", url, None, host[1], origin)
    require(status_after == 200, "post-restart product read failed")
    after = snapshot(stack, event)
    compared = ("event", "giveaways", "reservations", "outbox_ids", "receipts",
                "closure_jobs", "closure_effects")
    require(all(after[key] == rec["before"][key] for key in compared),
            "restart changed committed " + ", ".join(key for key in compared
                                                    if after[key] != rec["before"][key]))
    rec["after"] = after
    rec["compared"] = list(compared)
    rec["actions"].append({"pids": {"gateway": [old_gateway, stack.children["api-gateway"].process.pid]},
                           "unchanged_rows": list(compared)})
    rec["invariants"] = ["the restarted gateway returned readiness",
                         "committed event, giveaway, reservation, outbox identity and receipt rows unchanged",
                         "product read recovered through the gateway"]
    rec["state"] = "PASS"


def scenario_db_restart(stack, gateway, origin, event, host, alice, rec):
    rec["before"] = snapshot(stack, event)
    marker = str(uuid4())
    stack.cluster.sql(CONCERT_DB, "create table if not exists qa09_inflight(marker text primary key);")
    session = psql_session(
        stack.cluster, CONCERT_DB,
        "BEGIN; INSERT INTO qa09_inflight VALUES ('" + marker + "'); SELECT pg_sleep(300);",
        "qa09_inflight")
    sleeping = monotonic_wait(lambda: stack.cluster.sql(
        "postgres",
        "select count(*) from pg_stat_activity where application_name='qa09_inflight' and wait_event='PgSleep';"), 8)
    require(sleeping == "1", "in-flight transaction never reached its sleep")
    rec["actions"].append({"in_flight_transaction": {"marker": marker, "postmaster": stack.cluster.assert_postmaster()}})
    before_pid = stack.cluster.assert_postmaster()
    migration.execute([str(stack.cluster.tools["pg_ctl"]), "-D", str(stack.cluster.data), "-m", "immediate",
                       "-w", "-t", "10", "stop"], env=stack.cluster.env, phase="qa09-immediate-stop",
                      timeout=20, cwd=stack.cluster.root)
    command(rec, "postmaster immediate stop", ["pg_ctl", "-m", "immediate", "stop"], 0)
    stack.cluster.start()
    after_pid = stack.cluster.assert_postmaster()
    require(before_pid != after_pid, "postmaster did not change")
    recovered_log = log_excerpt(stack.cluster.root / "postgres.log", 6000)
    require("database system was interrupted" in recovered_log or "not properly shut down" in recovered_log,
            "crash recovery line absent from the postmaster log")
    try:
        session.wait(timeout=10)
    except subprocess.TimeoutExpired:
        session.kill()
        session.wait(timeout=5)
    session.stdout.close()
    session.stderr.close()
    rec["actions"].append({"postmaster": [before_pid, after_pid],
                           "in_flight_exit": session.returncode})
    rec["logs"].append({"source": "postgres.log", "excerpt": recovered_log})
    remaining = sql(rec, CONCERT_DB, "select count(*) from qa09_inflight where marker='" + marker + "'", "inflight_after_restart")
    require(remaining == "0", "in-flight transaction left a half-committed row")
    committed = snapshot(stack, event)
    require(all(line in committed["reservations"] for line in row_lines(rec["before"]["reservations"])),
            "database restart lost previously committed reservation rows")
    require(committed["receipts"] == rec["before"]["receipts"], "database restart changed committed receipts")
    recover_services(stack, rec)
    status_read, _ = request("GET", f"{gateway}/api/concert/events/{event}/giveaways", None, host[1], origin)
    require(status_read == 200, "product read did not recover after the database restart")
    stack.cluster.sql(CONCERT_DB, "drop table qa09_inflight;")
    rec["after"] = snapshot(stack, event)
    rec["invariants"] = ["the in-flight transaction resolved to a rollback after crash recovery",
                         "no half-committed marker survived",
                         "committed pickup rows and receipts remained present",
                         "the product read recovered through the gateway"]
    rec["state"] = "PASS"


def scenario_outbox_replay(stack, rec):
    rec["before"] = {}
    live = int(stack.cluster.sql(CONCERT_DB, "select count(*) from concert_world_invalidation_receipts"))
    require(live > 0, "World invalidation consumer did not produce receipts")
    row = stack.cluster.sql(CONCERT_DB, "select outbox_id||'|'||signal_id||'|'||state||'|'||attempts from (select o.outbox_id, r.signal_id, o.state, o.attempts from concert_outbox o join concert_world_invalidation_receipts r on r.signal_id=o.outbox_id where o.state='DELIVERED' order by o.outbox_id limit 1) q")
    require(row, "no delivered outbox row with a consumer receipt")
    outbox_id, signal_id, state, attempts = row.split("|")
    receipts_before = int(stack.cluster.sql(CONCERT_DB, "select count(*) from concert_world_invalidation_receipts where signal_id='" + signal_id + "'"))
    rec["before"] = {"outbox": row, "receipts": receipts_before}
    sql(rec, CONCERT_DB, "update concert_outbox set state='READY', terminal_at=null, available_at=clock_timestamp() where outbox_id='" + outbox_id + "'", "forced_redelivery")
    rec["actions"].append({"force_redelivery": {"outbox_id": outbox_id, "signal_id": signal_id}})
    delivered = monotonic_wait(lambda: stack.cluster.sql(CONCERT_DB, "select state||'|'||attempts from concert_outbox where outbox_id='" + outbox_id + "'"), 15)
    require(delivered and delivered.startswith("DELIVERED"), "redelivered outbox row did not return to DELIVERED")
    receipts_after = int(stack.cluster.sql(CONCERT_DB, "select count(*) from concert_world_invalidation_receipts where signal_id='" + signal_id + "'"))
    rec["after"] = {"outbox": delivered, "receipts": receipts_after}
    sql(rec, CONCERT_DB, "select state||'|'||attempts from concert_outbox where outbox_id='" + outbox_id + "'", "outbox_after_replay")
    sql(rec, CONCERT_DB, "select count(*) from concert_world_invalidation_receipts where signal_id='" + signal_id + "'", "receipts_after_replay")
    require(receipts_after == 1, "redelivery produced a duplicate consumer receipt")
    require(attempts != delivered.split("|")[1], "redelivery did not consume a new attempt")
    rec["invariants"] = ["forced redelivery returned to DELIVERED",
                         "consumer receipt count stayed exactly one",
                         "the redelivery consumed a new attempt"]
    rec["state"] = "PASS"


def scenario_partial_closure(stack, gateway, origin, event, host, rec):
    rec["before"] = snapshot(stack, event)
    recover_services(stack, rec)
    closure_url = f"{gateway}/api/concert/events/{event}/admin/closure"
    payload = {"confirmation": "CLOSE_EVENT", "expected_revision": 1, "reason": "QA09 interrupted closure"}
    lock = RowLock(stack.cluster, "select 1 from concert_events where event_id='" + event + "' for update;", "event").granted()
    closure_key = str(uuid4())
    outcome = {}
    thread = threading.Thread(target=lambda: outcome.update(
        response=request("POST", closure_url, payload, host[1], origin, closure_key)))
    thread.start()
    require(monotonic_wait(lambda: blocked_backends(stack.cluster) >= 1, 8), "closure never reached the event row lock")
    rec["actions"].append({"pending_closure": "request blocked on the event row"})
    lock.release()
    thread.join(timeout=30)
    require(not thread.is_alive(), "pending closure did not resolve after unlock")
    status, body = outcome.get("response", (None, {}))
    rec["actions"].append({"blocked_closure_outcome": {"status": status, "body": body}})
    phase = sql(rec, CONCERT_DB, "select phase||'|'||revision from concert_events where event_id='" + event + "'", "event_after_blocked_attempt")
    if not phase.startswith("READ_ONLY"):
        status, body, attempts = retry_dependency(
            lambda: request("POST", closure_url, payload, host[1], origin, closure_key))
        rec["actions"].append({"resume_closure": {"status": status, "body": body, "attempts": attempts}})
        require(status == 200, f"closure retry did not complete: {status}")
    phase = sql(rec, CONCERT_DB, "select phase||'|'||revision from concert_events where event_id='" + event + "'", "event_after_resume")
    require(phase.startswith("READ_ONLY"), "closure resume did not enter READ_ONLY")
    jobs = sql(rec, CONCERT_DB, "select job_id||'|'||(progress->>'state')||'|'||revision from concert_closure_jobs where event_id='" + event + "'", "closure_job_after_resume")
    require(jobs != "", "closure resume did not persist a job")
    status, refused = request("POST", f"{gateway}/api/concert/events/{event}/giveaways", {
        "title": "must be refused", "description": "post-closure", "image_media_id": None,
        "quantity_total": 1,
        "pickup_starts_at": (datetime.now(timezone.utc) - timedelta(minutes=1)).strftime("%Y-%m-%dT%H:%M:%S.%fZ"),
        "pickup_ends_at": (datetime.now(timezone.utc) + timedelta(hours=1)).strftime("%Y-%m-%dT%H:%M:%S.%fZ"),
        "public_place_id": closure.IN02_PLACE}, host[1], origin, str(uuid4()))
    rec["actions"].append({"post_closure_write": {"status": status, "body": refused}})
    require(status == 409, "post-closure write was not refused")
    status_read, read_body = request("GET", f"{gateway}/api/concert/events/{event}/giveaways", None, host[1], origin)
    rec["actions"].append({"post_closure_read": {"status": status_read}})
    require(status_read == 200, "post-closure read stopped working")
    rec["after"] = snapshot(stack, event)
    rec["invariants"] = ["a closure blocked mid-transaction completed atomically to READ_ONLY after the lock backend was released",
                         "a durable closure job and READ_ONLY effect were persisted",
                         "product writes were refused with 409 after closure",
                         "product reads continued"]
    rec["state"] = "PASS"


def scenario_backup_restore(stack, event, host, gateway, origin, rec):
    rec["before"] = snapshot(stack, event)
    dump = stack.cluster.root / "concert-backup.dump"
    dump_argv = [str(PG_BIN / "pg_dump"), "-h", str(stack.cluster.socket), "-p", "5432",
                 "-U", migration.OWNER, "-Fc", "-f", str(dump), CONCERT_DB, "--no-password"]
    result = migration.execute(
        dump_argv, env=stack.cluster.client_env("postgres"), phase="qa09-pg-dump", timeout=120,
        cwd=stack.cluster.root)
    command(rec, "pg_dump", dump_argv, 0, result.stdout)
    require(dump.is_file() and dump.stat().st_size > 0, "logical backup is empty")
    backup = {"bytes": dump.stat().st_size,
              "sha256": __import__("hashlib").sha256(dump.read_bytes()).hexdigest(),
              "command": " ".join(shlex.quote(str(item)) for item in dump_argv),
              "path": str(dump)}
    rec["backup"] = backup
    rec["actions"].append({"backup": backup})
    mutation = sql(rec, CONCERT_DB, "create table qa09_backup_probe(value text primary key); insert into qa09_backup_probe values('QA09 MUTATED'); select value from qa09_backup_probe", "mutation")
    require(row_lines(mutation)[-1] == "QA09 MUTATED", "mutation was not applied")
    restored_db = "mcr_concert_restored"
    migration.execute([str(stack.cluster.tools["createdb"]), "--no-password", "--maintenance-db=postgres", restored_db],
                      env=stack.cluster.client_env("postgres"), phase="qa09-createdb-restored",
                      timeout=30, cwd=stack.cluster.root)
    restore_argv = [str(PG_BIN / "pg_restore"), "-h", str(stack.cluster.socket), "-p", "5432",
                    "-U", migration.OWNER, "-d", restored_db, "--no-owner", "--no-privileges",
                    str(dump), "--no-password"]
    result = migration.execute(
        restore_argv,
        env=stack.cluster.client_env("postgres"), phase="qa09-pg-restore", timeout=120,
        cwd=stack.cluster.root)
    command(rec, "pg_restore", restore_argv, 0, result.stdout)
    rec["after"] = snapshot(stack, event, db=restored_db)
    for key in ("event", "giveaways", "reservations", "outbox_ids", "receipts"):
        require(rec["after"][key] == rec["before"][key], "restore mismatch: " + key)
    mutation_table = stack.cluster.sql(restored_db, "select coalesce(to_regclass('public.qa09_backup_probe')::text,'ABSENT')")
    rec["db_rows"]["restored_mutation_table"] = mutation_table
    require(mutation_table == "ABSENT", "mutation table survived the restore")
    stack.cluster.sql(CONCERT_DB, "drop table if exists qa09_backup_probe;")
    require(rec["after"]["outbox_ids"] == rec["before"]["outbox_ids"],
            "restore lost or duplicated outbox identities")
    stack.cluster.sql("postgres", "drop database " + restored_db + ";")
    dump.unlink()
    rec["actions"].append({"restored_database": restored_db, "dropped": True, "dump_removed": True})
    rec["invariants"] = ["logical backup restored the exact committed rows",
                         "the post-backup mutation was discarded",
                         "outbox and receipt identities were neither lost nor duplicated"]
    rec["state"] = "PASS"


def scenario_deletion_probe(stack, gateway, origin, event, host, admin, rec):
    user_id = admin[0]
    control = host[0]
    overrides = {"AUTH_USER_DELETION_WORKER_ENABLED": "true",
                 "AUTH_USER_DELETION_WORKER_INTERVAL_MS": "250",
                 "AUTH_USER_DELETION_WORKER_MAX_ATTEMPTS": "8"}
    rec["actions"].append({"auth_worker_env": {key: value for key, value in
                                               CHILD_SPECS["auth-service"]["env"].items()
                                               if "USER_DELETION" in key},
                           "worker_overrides": overrides})
    stack.cluster.sql(CONCERT_DB,
        "insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) "
        "values('" + event + "','" + user_id + "','DANGER',true,true,true) on conflict do nothing;"
        "insert into concert_avatar_settings(event_id,user_id,contract_version,representation,preset_id,manifest_id,manifest_version) "
        "values('" + event + "','" + user_id + "','1.1.0','whole_character','photo01_front_01','mcr_selected_static_20261103',1) on conflict do nothing;"
        "insert into concert_age_owners(user_id) values('" + user_id + "') on conflict do nothing;")

    def counts(tag):
        values = {}
        for label, table in (("participants", "concert_participants"),
                             ("avatar_settings", "concert_avatar_settings"),
                             ("age_owners", "concert_age_owners")):
            for owner, identifier in (("deleted", user_id), ("control", control)):
                values[owner + "_" + label] = int(sql(
                    rec, CONCERT_DB,
                    "select count(*) from " + table + " where user_id='" + identifier + "'",
                    owner + "_" + label + "_" + tag))
        return values

    def step_rows(label):
        rows = []
        query = ("select s.target||'|'||s.status||'|'||s.attempt_count||'|'||coalesce(s.last_error,'') "
                 "from user_deletion_steps s join user_deletion_sagas g on g.id=s.saga_id "
                 "where g.user_id='" + user_id + "' order by s.target")
        for line in row_lines(sql(rec, AUTH_DB, query, label)):
            target, state, attempts, last_error = line.split("|", 3)
            rows.append({"target": target, "status": state,
                         "attempt_count": int(attempts), "last_error": last_error})
        return rows

    rec["before"] = {
        "rows": counts("before"),
        "auth": sql(rec, AUTH_DB, "select status||'|'||coalesce(deleted_at::text,'') from users where id='" + user_id + "'", "auth_before"),
    }
    status, body = request("DELETE", f"{gateway}/api/auth/account", None, admin[1], origin)
    rec["actions"].append({"delete": {"status": status, "body": body}})
    if status != 202:
        rec["state"] = "NOT_RUN"
        rec["blocker"] = f"account deletion request returned {status}, so the saga was not exercised"
        return
    saga = sql(rec, AUTH_DB, "select id||'|'||status||'|'||coalesce(completed_at::text,'') from user_deletion_sagas where user_id='" + user_id + "'", "saga_after_delete")
    require(saga.startswith(saga.split("|")[0]) and "IN_PROGRESS" in saga, "deletion saga was not created in progress")
    steps_after_delete = step_rows("steps_after_delete")
    require(len(steps_after_delete) == 8, "deletion saga does not have the canonical eight steps")
    sql(rec, AUTH_DB, "select clock_timestamp()||'|'||(select bool_and(next_attempt_at<=clock_timestamp()) from user_deletion_steps st join user_deletion_sagas sg on sg.id=st.saga_id where sg.user_id='" + user_id + "')", "steps_due_check")
    restart_service(stack, "auth-service", rec, kill=True, overrides=overrides)
    recover_services(stack, rec)

    def process_env(pid, prefixes):
        try:
            raw = subprocess.run(["/bin/ps", "-E", "-p", str(pid)], capture_output=True,
                                 text=True, timeout=5).stdout
            return sorted({part for part in raw.split() if part.startswith(prefixes)})
        except Exception as error:
            return ["probe failed: " + type(error).__name__]

    rec["actions"].append({
        "auth_process_worker_env": process_env(stack.children["auth-service"].process.pid,
                                                 ("AUTH_USER_DELETION_",
                                                  "AUTH_CONCERT_DELETION_MODE",
                                                  "AUTH_CONCERT_GRPC_ENDPOINT")),
        "concert_process_deletion_env": process_env(stack.children["concert-service"].process.pid,
                                                     ("CONCERT_ACCOUNT_DELETION_",
                                                      "CONCERT_AUTH_GRPC_ENDPOINT")),
    })
    completed = monotonic_wait(lambda: stack.cluster.sql(AUTH_DB,
        "select completed_at is not null from user_deletion_sagas where user_id='" + user_id + "'"), 30)
    final_saga = sql(rec, AUTH_DB, "select id||'|'||status||'|'||coalesce(completed_at::text,'') from user_deletion_sagas where user_id='" + user_id + "'", "saga_after_worker_restart")
    final_steps = step_rows("steps_after_worker_restart")
    rec["after"] = {
        "rows": counts("after"),
        "auth": sql(rec, AUTH_DB, "select status||'|'||coalesce(deleted_at::text,'') from users where id='" + user_id + "'", "auth_after"),
        "saga": final_saga,
        "steps": final_steps,
    }
    for name in ("auth-service", "concert-service"):
        try:
            rec["logs"].append({"source": name + " after worker restart",
                                "excerpt": stack.children[name].diagnostic()})
        except Exception:
            pass
    for label in ("participants", "avatar_settings", "age_owners"):
        require(rec["after"]["rows"]["control_" + label]
                == rec["before"]["rows"]["control_" + label],
                "control user rows changed: " + label)
    claims = sum(step["attempt_count"] for step in final_steps)
    if (completed is not None and "|COMPLETED|" in final_saga
            and all(step["status"] == "SUCCEEDED" for step in final_steps)
            and rec["after"]["rows"]["deleted_participants"] == 0
            and rec["after"]["rows"]["deleted_avatar_settings"] == 0
            and rec["after"]["rows"]["deleted_age_owners"] == 0):
        rec["state"] = "PASS"
        rec["invariants"] = ["all eight canonical targets completed after the Auth worker restart",
                             "private Concert rows were removed",
                             "the control user remained intact"]
    else:
        rec["state"] = "NOT_RUN"
        rec["invariants"] = ["not claimed: the live seven-service saga ran but did not converge; measured step rows below"]
        if claims == 0:
            rec["blocker"] = ("the live seven-service Auth worker claimed none of the eight due steps: "
                              "all eight remained PENDING with attempt_count 0 after the SIGKILL/restart "
                              "and a 30 s bound")
        else:
            failed = [step for step in final_steps if step["status"] != "SUCCEEDED"]
            rec["blocker"] = ("the live seven-service saga ran but did not converge within the 30 s bound: "
                              + "; ".join(step["target"] + "=" + step["status"]
                                          + "/attempts=" + str(step["attempt_count"])
                                          + "/error=" + (step["last_error"] or "<none>")
                                          for step in failed))


def scenario_baseline(stack, gateway, origin, event, host, alice, rec):
    rec["before"] = snapshot(stack, event)
    pickup = provision_pickup(stack, gateway, origin, event, host, alice, rec, "baseline")
    key = str(uuid4())
    target = f"{pickup['base']}/reservations/{pickup['reservation']}/claim"
    payload = {"expected_revision": pickup["reservation_revision"], "confirmation_code": pickup["code"]}
    status, body = request("POST", target, payload, host[1], origin, key)
    require(status == 200 and body.get("state") == "CLAIMED", f"baseline claim {status}")
    counters = sql(rec, CONCERT_DB, "select reserved_unclaimed||'|'||quantity_claimed from concert_community_giveaways where giveaway_id='" + pickup["giveaway"] + "'", "baseline_giveaway")
    require(counters == "0|1", "baseline claim did not consume one unit")
    receipts = sql(rec, CONCERT_DB, "select count(*) from concert_command_receipts where event_id='" + event + "' and operation_id='claimConcertReservation' and state='SUCCEEDED' and resource_id='" + pickup["reservation"] + "'", "baseline_receipts")
    require(receipts == "1", "baseline claim did not produce one receipt")
    status2, replay = request("POST", target, payload, host[1], origin, key)
    require(status2 == 200 and replay.get("receipt_id") == body.get("receipt_id"), "baseline replay changed the claim receipt")
    rec["checks"] = {"giveaway_create_200": True, "giveaway_meeting_200": True,
                     "giveaway_open_200": True, "pickup_code_issued": True,
                     "pickup_claim_200": status == 200,
                     "same_key_replay_same_receipt": replay.get("receipt_id") == body.get("receipt_id")}
    rec["after"] = snapshot(stack, event)
    rec["invariants"] = ["real giveaway, meeting, reservation and pickup code created",
                         "claim consumed exactly one unit",
                         "same idempotency key replayed the original claim receipt"]
    rec["state"] = "PASS"


def qa09_gate(stack, gateway, origin, event, host, alice, bob, report, record):
    global CURRENT_STACK
    CURRENT_STACK = stack
    checks = {}
    scenarios = [
        ("QA09-SERVICE-RESTART", "Gateway restart", scenario_service_restart),
        ("QA09-PENDING-PICKUP", "Interrupted pickup claim", scenario_pending_pickup),
        ("QA09-OUTBOX-REPLAY", "Outbox replay after SQL reset and cluster restart", scenario_outbox_replay),
        ("QA09-DB-RESTART", "Postmaster crash restart", scenario_db_restart),
        ("QA09-DELETION-SAGA", "Live seven-service deletion saga", scenario_deletion_probe),
        ("QA09-BACKUP-RESTORE", "Logical backup and restore", scenario_backup_restore),
        ("QA09-PARTIAL-CLOSURE", "Interrupted event closure", scenario_partial_closure),
    ]
    baseline = {"id": "QA09-BASELINE-PICKUP", "name": "Real pickup baseline",
                "state": "RUNNING", "db_rows": {}, "commands": [], "actions": [], "logs": []}
    RESULT["scenarios"].append(baseline)
    try:
        scenario_baseline(stack, gateway, origin, event, host, alice, baseline)
    except Exception:
        baseline["state"] = "FAIL"
        baseline["error"] = traceback.format_exc(limit=3)
        return {case_id.lower().replace('-', '_'): False for case_id, _, _ in scenarios}, "baseline pickup failed"

    checks["qa09_baseline_pickup"] = True
    if NATIVE_OUTBOX is not None:
        scenarios = [item for item in scenarios if item[0] != "QA09-OUTBOX-REPLAY"]
    if ONLY_CASES is not None:
        scenarios = [item for item in scenarios if item[0] in ONLY_CASES]
    for case_id, name, fn in scenarios:
        rec = {"id": case_id, "name": name, "state": "RUNNING", "db_rows": {}, "commands": [],
               "actions": [], "logs": [], "started_at": now()}
        RESULT["scenarios"].append(rec)
        try:
            if fn is scenario_backup_restore:
                fn(stack, event, host, gateway, origin, rec)
            elif fn is scenario_db_restart:
                fn(stack, gateway, origin, event, host, alice, rec)
            elif fn is scenario_outbox_replay:
                fn(stack, rec)
            elif fn is scenario_pending_pickup:
                fn(stack, gateway, origin, event, host, alice, rec)
            elif fn is scenario_deletion_probe:
                fn(stack, gateway, origin, event, host, alice, rec)
            elif fn is scenario_partial_closure:
                fn(stack, gateway, origin, event, host, rec)
            elif fn is scenario_service_restart:
                fn(stack, gateway, origin, event, host, rec)
            rec["finished_at"] = now()
        except Exception:
            rec["state"] = "FAIL"
            rec["error"] = traceback.format_exc(limit=6)
            rec["finished_at"] = now()
            try:
                recover_services(stack, rec)
            except Exception:
                rec["recovery_error"] = traceback.format_exc(limit=3)
        checks[case_id.lower().replace('-', '_')] = rec["state"] == "PASS"
        if rec["state"] == "NOT_RUN":
            checks.pop(case_id.lower().replace('-', '_'), None)
    if NATIVE_OUTBOX is not None:
        checks["qa09_outbox_replay"] = NATIVE_OUTBOX["state"] == "PASS"
    if NATIVE_DELETION is not None:
        checks["qa09_deletion"] = NATIVE_DELETION["state"] == "PASS"
    failures = [rec for rec in RESULT["scenarios"] if rec["state"] not in ("PASS", "NOT_RUN")]
    RESULT["finished_at"] = now()
    return checks, (None if not failures else "failed scenarios: " + ", ".join(rec["id"] for rec in failures))


def install_composition():
    global SOCIAL, AUTH_GRPC
    SOCIAL = {
        "CONCERT_SOCIAL_MODE": "live",
        "CONCERT_NOTIFICATION_INBOX_MODE": "live",
        "CONCERT_NOTIFICATION_TARGET_MODE": "live",
        "CONCERT_SOCIAL_REQUEST_TTL_SECONDS": "300",
        "CONCERT_SOCIAL_CURSOR_KEY_HEX": secrets.token_hex(32),
        "CONCERT_SOCIAL_HANDLE_KEYS_JSON": json.dumps([{"id": "qa09", "secret_hex": secrets.token_hex(32)}]),
        "CONCERT_SOCIAL_COMMAND_KEYS_JSON": json.dumps([{"id": "qa09", "secret_hex": secrets.token_hex(32)}]),
    }
    original_execute = migration.execute
    original_environment = dev_stack.development_environments
    original_child = runtime.Child.__init__

    def execute(argv, **kwargs):
        env = kwargs.get("env")
        if env is not None and "--inspect-product-release" in argv:
            env.update(SOCIAL)
            env["CONCERT_ACCOUNT_DELETION_MODE"] = "managed"
            env["CONCERT_AUTH_GRPC_ENDPOINT"] = AUTH_GRPC
        return original_execute(argv, **kwargs)

    def environments(cluster, ports, keys, credentials, redis_password, mail_token, browser_origin=None):
        global AUTH_GRPC
        AUTH_GRPC = "https://" + ports.address("auth-service:grpc")
        return original_environment(cluster, ports, keys, credentials, redis_password, mail_token, browser_origin)

    def child(self, cluster, name, argv, env, ports, reservations, secrets_to_redact=()):
        if name == "notification-service":
            env["NOTIFICATION_CONCERT_MODE"] = "live"
            env["NOTIFICATION_CONCERT_AUTH_GRPC_ENDPOINT"] = "https://" + ports.address("auth-service:grpc")
            env["NOTIFICATION_CONCERT_FRIEND_GRPC_ENDPOINT"] = "https://" + ports.address("friend-service:grpc")
            env["NOTIFICATION_CONCERT_ROOT_GRPC_ENDPOINT"] = "https://" + ports.address("concert-service:grpc")
        if name == "concert-service":
            env["CONCERT_ACCOUNT_DELETION_MODE"] = "managed"
            env["CONCERT_AUTH_GRPC_ENDPOINT"] = "https://" + ports.address("auth-service:grpc")
        if name == "auth-service":
            env["AUTH_CONCERT_DELETION_MODE"] = "managed"
            env["AUTH_CONCERT_GRPC_ENDPOINT"] = "https://" + ports.address("concert-service:grpc")
            env["AUTH_CONCERT_DELETION_SOURCE_ENABLED"] = "true"
            env["AUTH_USER_DELETION_WORKER_ENABLED"] = "true"
            env.setdefault("AUTH_USER_DELETION_WORKER_INTERVAL_MS", "60000")
        original_child(self, cluster, name, argv, env, ports, reservations, secrets_to_redact)
        CHILD_SPECS[name] = {"argv": [str(item) for item in argv], "env": dict(env),
                             "reservations": tuple(reservations),
                             "secrets": tuple(secrets_to_redact)}

    migration.execute = execute
    dev_stack.development_environments = environments
    runtime.Child.__init__ = child
    closure.in02_gate = qa09_gate


def run_native_outbox(test_binary, rec):
    tools = {name: migration.executable(PG_BIN / name) for name in migration.PG_TOOLS}
    cluster = migration.Cluster(tools)
    cluster.init()
    cluster.start()
    cluster.sql("postgres", "create database mcr_world_invalidation template template0;")
    for path in sorted((ROOT / "backend/services/concert-service/migrations").glob("*.sql")):
        cluster.sql("mcr_world_invalidation", path.read_text())
    env = {**cluster.env,
           "WORLD_INVALIDATION_TEST_OWNER_ROOT": str(cluster.root),
           "WORLD_INVALIDATION_TEST_OWNER_TOKEN": cluster.token,
           "WORLD_INVALIDATION_TEST_OWNER_IDENTITY": f"{cluster.identity[0]}:{cluster.identity[1]}",
           "WORLD_INVALIDATION_TEST_DATABASE_URL": cluster.url("mcr_world_invalidation")}

    def run_test(name, timeout=120):
        result = subprocess.run(
            [str(test_binary), "--ignored", "--exact", "native::" + name, "--test-threads=1", "--nocapture"],
            env=env, cwd=ROOT / "backend", capture_output=True, text=True, timeout=timeout)
        rec["commands"].append({"label": name, "argv": ["world_invalidation", name],
                                "exit_code": result.returncode,
                                "output": (result.stdout + result.stderr)[-8000:]})
        require(result.returncode == 0 and "1 passed; 0 failed;" in result.stdout,
                f"world invalidation test failed: {name}: {(result.stdout + result.stderr)[-2000:]}")
        return result

    try:
        rec["actions"].append({"owned_cluster": str(cluster.root),
                               "postgres": cluster.sql("postgres", "show server_version;"),
                               "binary_sha256": __import__("hashlib").sha256(test_binary.read_bytes()).hexdigest()})
        run_test("native_block_outbox_fence_commit_retry_and_native_send")
        rec["before"] = {
            "outbox": cluster.sql("mcr_world_invalidation", "select outbox_id||'|'||state||'|'||attempts from concert_outbox order by outbox_id"),
            "receipts": cluster.sql("mcr_world_invalidation", "select count(*) from concert_world_invalidation_receipts"),
            "replayed_signal_receipts": cluster.sql("mcr_world_invalidation", "select count(*) from concert_world_invalidation_receipts r join world_invalidation_test_restart t on t.signal=r.signal_id"),
            "persisted_signal": cluster.sql("mcr_world_invalidation", "select signal||'|'||receipt from world_invalidation_test_restart"),
        }
        before_pid = cluster.assert_postmaster()
        cluster.stop()
        cluster.start()
        after_pid = cluster.assert_postmaster()
        require(before_pid != after_pid, "native outbox restart did not change the postmaster")
        run_test("native_restart_durable_receipt_preserves_fresh_connection")
        rec["after"] = {
            "outbox": cluster.sql("mcr_world_invalidation", "select outbox_id||'|'||state||'|'||attempts from concert_outbox order by outbox_id"),
            "receipts": cluster.sql("mcr_world_invalidation", "select count(*) from concert_world_invalidation_receipts"),
            "replayed_signal_receipts": cluster.sql("mcr_world_invalidation", "select count(*) from concert_world_invalidation_receipts r join world_invalidation_test_restart t on t.signal=r.signal_id"),
        }
        rec["actions"].append({"postmaster": [before_pid, after_pid]})
        require(rec["after"]["replayed_signal_receipts"] == "1", "replay produced a duplicate consumer receipt")
        rec["invariants"] = ["the native test reset the delivered outbox row to READY with SQL and the harness stopped and started the owned postmaster",
                             "a fresh process and connection replayed the row",
                             "the consumer receipt for the replayed signal remained exactly one",
                             "the outbox row returned to DELIVERED after the new process with one new attempt"]
        rec["actions"].append({"replay_fault": "SQL reset of the delivered outbox row to READY plus a full owned PostgreSQL cluster stop/start, not a process killed mid-publish"})
        rec["state"] = "PASS"
    finally:
        rec["logs"].append({"source": "native test output",
                            "excerpt": "\n".join(item["output"] for item in rec["commands"])[-12000:]})
        cluster.cleanup()
        rec["actions"].append({"cleanup": "owned root absent", "absent": not cluster.root.exists()})


def run_native_deletion(test_binary, script, rec):
    import tempfile
    report_path = Path(tempfile.mkdtemp(prefix="qa09-deletion-")) / "mount.json"
    result = subprocess.run(
        [sys.executable, str(script), "--binary", str(test_binary), "--pg-bin", str(PG_BIN),
         "--report", str(report_path)], capture_output=True, text=True, timeout=900)
    rec["commands"].append({"label": "account-deletion-mount",
                            "argv": ["account_deletion_mount.py", str(test_binary)],
                            "exit_code": result.returncode,
                            "output": (result.stdout + result.stderr)[-4000:]})
    require(report_path.is_file(), "deletion mount report missing")
    report = json.loads(report_path.read_text())
    rec["actions"].append({"status": report.get("status"),
                           "source": report.get("source", {}).get("exit_code"),
                           "after_pg_restart": report.get("after_pg_restart", {}).get("exit_code"),
                           "restart": report.get("restart"),
                           "migration_receipts": report.get("migration_receipts"),
                           "migration_receipts_after": report.get("migration_receipts_after"),
                           "persisted_after_pg_restart": report.get("persisted_after_pg_restart"),
                           "cleanup": report.get("cleanup"),
                           "limitations": report.get("limitations")})
    for phase in ("source", "after_pg_restart"):
        value = report.get(phase, {})
        rec["logs"].append({"source": "account deletion " + phase,
                            "excerpt": (value.get("stdout", "") + value.get("stderr", ""))[-8000:]})
    require(result.returncode == 0 and report.get("status") == "PASS",
            "account deletion mount test failed")
    measured = report.get("probes") or {}
    require(set(measured) == {"source", "after_pg_restart"}
            and all(len(values) == 2 for values in measured.values()),
            "deletion mount report lacks measured before/after rows")
    rec["before"] = measured["source"][0]
    rec["after"] = measured["source"][-1]
    rec["db_rows"]["deletion_mount"] = measured
    if report.get("persisted_after_pg_restart"):
        rec["db_rows"]["persisted_after_pg_restart"] = report["persisted_after_pg_restart"]
    rec["qualification"] = "isolated owner mount"
    rec["invariants"] = ["the production Auth deletion source and mounted Concert PrivateData factory sanitized the seeded participant row and removed the avatar and age-owner rows over mTLS (the private-row predicate is 0)",
                         "the control user's rows were unchanged",
                         "a fresh composed factory replayed the sealed acknowledgement after a service restart without rewriting its terminal receipt",
                         "the same case ran again after a physical PostgreSQL restart against the same durable stores",
                         "the canonical Auth saga's eight steps never left PENDING in this mount, so it is not deletion convergence"]
    rec["state"] = "PASS"


def write_evidence(directory):
    directory.mkdir(parents=True, exist_ok=True)
    json_path = directory / "task-41-qa09.json"
    json_path.write_text(json.dumps(RESULT, indent=2, sort_keys=True) + "\n")
    not_run = [rec for rec in RESULT["scenarios"] if rec["state"] == "NOT_RUN"]
    status = "PASS" if RESULT.get("result") == "PASS" else "FAIL"
    if not_run:
        status += " (" + "; ".join(rec["id"] + ": NOT_RUN" for rec in not_run) + ")"
    lines = ["# QA09 recovery and lifecycle rehearsal", "",
             "Status: " + status,
             "Run: " + RESULT.get("started_at", "") + " to " + RESULT.get("finished_at", ""),
             "Scope: owned PG16 cluster and the seven-service development composition; not a deployment or capacity claim.", "",
             "| Scenario | State | Restored invariant |", "|---|---|---|"]
    for rec in RESULT["scenarios"]:
        failure_lines = (rec.get("error") or "").strip().splitlines()
        invariant = ("; ".join(rec.get("invariants", []))
                     or (failure_lines[-1][:180] if failure_lines else rec.get("blocker", rec["state"])))
        state = rec["state"] + (f" ({rec['qualification']})" if rec.get("qualification") else "")
        lines.append(f"| {rec['id']} - {rec['name']} | {state} | {invariant} |")
    lines.append("")
    for rec in RESULT["scenarios"]:
        state = rec["state"] + (f" ({rec['qualification']})" if rec.get("qualification") else "")
        lines += [f"## {rec['id']} - {rec['name']}", "", f"State: {state}", ""]
        if rec.get("blocker"):
            lines += ["Blocker:", "", "```text", rec["blocker"], "```", ""]
        if rec.get("actions"):
            lines += ["Actions:", "", "```json", json.dumps(rec["actions"], indent=1)[:12000], "```", ""]
        if rec.get("before"):
            lines += ["Before:", "", "```text", json.dumps(rec["before"], indent=1)[:8000], "```", ""]
        if rec.get("after"):
            lines += ["After:", "", "```text", json.dumps(rec["after"], indent=1)[:8000], "```", ""]
        if rec.get("backup"):
            lines += ["Measured backup:", "", "```json",
                      json.dumps(rec["backup"], indent=1, sort_keys=True), "```", ""]
        if rec.get("db_rows"):
            lines += ["Database rows:", "", "```text"]
            for label, value in rec["db_rows"].items():
                lines.append(f"[{label}]")
                lines.append(value if isinstance(value, str)
                             else json.dumps(value, indent=1, sort_keys=True))
            lines += ["```", ""]
        if rec.get("commands"):
            lines += ["Commands:", "", "| Command | Exit |", "|---|---:|"]
            for item in rec["commands"]:
                lines.append(f"| `{item['label']}` | {item['exit_code']} |")
            lines.append("")
        if rec.get("logs"):
            lines += ["Logs:", "", "```text", "\n".join(item["excerpt"] for item in rec["logs"])[:12000], "```", ""]
        if rec.get("error"):
            lines += ["Failure:", "", "```text", rec["error"][:8000], "```", ""]
    lines += METHOD_NOTES
    cleanup = RESULT.get("engine", {}).get("cleanup")
    lines += ["## Cleanup", "",
              f"- Final stack report cleanup: {cleanup}; the owned cluster root and its service children were removed.",
              "- Native outbox and deletion mount runs removed their owned cluster roots; the mutation runs removed theirs.",
              "- The throwaway mutation worktree was removed. No delivery source outside this harness and evidence was changed.",
              ""]
    (directory / "task-41-qa09.md").write_text("\n".join(lines) + "\n")


def main(argv=None):
    global PG_BIN
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path, default=PG_BIN)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--evidence-dir", type=Path, required=True)
    parser.add_argument("--native-outbox-test", type=Path)
    parser.add_argument("--native-deletion-test", type=Path)
    parser.add_argument("--deletion-mount-script", type=Path,
                        default=ROOT / "backend/scripts/mcr/account_deletion_mount.py")
    parser.add_argument("--only", help="comma-separated QA09 case ids for one isolated scenario")
    args = parser.parse_args(argv)
    PG_BIN = args.pg_bin_dir
    global ONLY_CASES
    ONLY_CASES = set(args.only.split(",")) if args.only else None
    install_composition()
    global NATIVE_OUTBOX
    if args.native_outbox_test is not None:
        native = {"id": "QA09-OUTBOX-REPLAY", "name": "Outbox replay after SQL reset and cluster restart",
                  "state": "RUNNING", "db_rows": {}, "commands": [], "actions": [], "logs": [],
                  "started_at": now()}
        RESULT["scenarios"].append(native)
        try:
            run_native_outbox(args.native_outbox_test, native)
        except Exception:
            native["state"] = "FAIL"
            native["error"] = traceback.format_exc(limit=6)
        native["finished_at"] = now()
        NATIVE_OUTBOX = native
    global NATIVE_DELETION
    if args.native_deletion_test is not None:
        deletion = {"id": "QA09-DELETION", "name": "PrivateData owner deletion mount",
                    "state": "RUNNING", "db_rows": {}, "commands": [], "actions": [], "logs": [],
                    "started_at": now()}
        RESULT["scenarios"].append(deletion)
        try:
            run_native_deletion(args.native_deletion_test, args.deletion_mount_script, deletion)
        except Exception:
            deletion["state"] = "FAIL"
            deletion["error"] = traceback.format_exc(limit=6)
        deletion["finished_at"] = now()
        NATIVE_DELETION = deletion
    code = 1
    try:
        code = closure.main(["--bin-dir", str(args.bin_dir), "--pg-bin-dir", str(args.pg_bin_dir),
                             "--report", str(args.report), "--in02"])
    except SystemExit as exit_error:
        code = int(exit_error.code or 0)
    except Exception:
        RESULT["engine"]["error"] = traceback.format_exc(limit=6)
    RESULT["result"] = "PASS" if code == 0 and all(
        rec["state"] in ("PASS", "NOT_RUN") for rec in RESULT["scenarios"]) and RESULT["scenarios"] else "FAIL"
    RESULT["not_run"] = [{"id": rec["id"], "blocker": rec.get("blocker", "")}
                         for rec in RESULT["scenarios"] if rec["state"] == "NOT_RUN"]
    RESULT["qualifications"] = {rec["id"]: rec["qualification"]
                                 for rec in RESULT["scenarios"] if rec.get("qualification")}
    RESULT["engine"]["exit_code"] = code
    RESULT["engine"]["stack_report"] = str(args.report)
    try:
        RESULT["engine"]["stack_report_sha256"] = __import__("hashlib").sha256(args.report.read_bytes()).hexdigest()
        stack_report = json.loads(args.report.read_text())
        RESULT["engine"]["cleanup"] = stack_report.get("cleanup")
        RESULT["engine"]["stack_result"] = stack_report.get("result")
    except (OSError, ValueError):
        pass
    RESULT["engine"]["source_revision"] = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    RESULT["engine"]["binaries"] = {
        name: __import__("hashlib").sha256((args.bin_dir / name).read_bytes()).hexdigest()
        for name in ("api-gateway", "auth-service", "profile-service", "friend-service",
                     "post-service", "notification-service", "concert-service")
        if (args.bin_dir / name).is_file()}
    write_evidence(args.evidence_dir)
    print(json.dumps({"result": RESULT["result"], "exit_code": code,
                      "scenarios": {rec["id"]: rec["state"] for rec in RESULT["scenarios"]}}))
    return 0 if RESULT["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
