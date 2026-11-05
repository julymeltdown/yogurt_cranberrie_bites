#!/usr/bin/env python3
"""Two missing BE03 native email cases; all execution requires a finite lease.

The Rust tests are std-only native entry points. This controller reuses the exact
retained DevStack read-only, never replays its original 53-case exercise, and
never writes a peer checkout. Every case failure is a fixed redacted label.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import stat
import subprocess
import sys
import threading
import time
from uuid import UUID

sys.dont_write_bytecode = True
BASELINE = "689e4d27c67debcd05d7280394184619cb4aea3b"
COHORT_HEAD = "41fb33c54fc828fe8c766da79002c2d0952695d1"
RECEIPT = "1e54b1814690ba9edd9282f36e8a0771400b0789907097bd0a5e7ae653e49c88"
CASES = {
    "expired": "expired_email_verification_does_not_activate",
    "resend": "unverified_resend_delivers_once_and_new_verification_succeeds",
}
OWNER = "01a19f83-12cc-7f21-8ad3-55749f012d9b"
RESOURCE = Path("/Users/lhs/Desktop/mcr-evidence/backend-staffing-01a1a21d")
OWN_RECEIPT = RESOURCE / "owner-resource-receipts" / (OWNER + ".json")
class Refusal(Exception):
    pass

def require(ok, label):
    if not ok:
        raise Refusal(label)

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def private_json(path, value):
    path = Path(path)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")

def load_private(path):
    path = Path(path)
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid()
            and info.st_mode & 0o077 == 0, "private-file-ownership")
    return json.loads(path.read_bytes())

def atomic_json(path, value):
    path = Path(path)
    temp = path.with_name(path.name + "." + str(os.getpid()) + ".tmp")
    private_json(temp, value)
    os.replace(temp, path)

def process_table():
    result = {}
    raw = subprocess.check_output(["/bin/ps", "-ww", "-axo", "pid=,ppid=,rss=,lstart=,stat=,comm=,args="], text=True, timeout=4)
    # comm can contain spaces, so collect it separately by PID. Birth and argv
    # remain from one contemporaneous wide snapshot, never basename guesses.
    commands = {}
    for line in subprocess.check_output(["/bin/ps", "-axo", "pid=,comm="], text=True, timeout=4).splitlines():
        f = line.strip().split(None, 1)
        if len(f) == 2:
            commands[int(f[0])] = f[1]
    for line in raw.splitlines():
        f = line.strip().split(None, 9)
        if len(f) != 10:
            continue
        pid = int(f[0])
        command = f[9]
        executable = commands.get(pid, "")
        if executable and command.startswith(executable):
            command = command[len(executable):].lstrip()
        result[pid] = {"parent": int(f[1]), "rss": int(f[2])*1024,
                       "birth": " ".join(f[3:8]), "state": f[8],
                       "executable": executable, "command": command}
    return result

def actual_identity(pid, output=None):
    birth = " ".join(subprocess.check_output(["/bin/ps", "-p", str(pid), "-o", "lstart="], text=True, timeout=4).split())
    executable = subprocess.check_output(["/bin/ps", "-p", str(pid), "-o", "comm="], text=True, timeout=4).strip()
    require(bool(birth and executable), "bootstrap-identity")
    value = {"pid": pid, "birth": birth, "executable": executable,
             "script": str(Path(__file__).resolve()), "cwd": os.getcwd()}
    if output:
        value["outputRoot"] = str(output)
    return value

def publish_accept(l, controller, guards, phase):
    value = json.loads(OWN_RECEIPT.read_bytes()) if OWN_RECEIPT.exists() else {}
    if value.get("kind") != "ordinary" and value.get("state", "").startswith("ACCEPT"):
        require(value.get("offerId") == l["allocation_id"] and value.get("acceptedAtEpoch") == l["accepted_epoch"], "own-acceptance-no-reset")
    value.update(ownerThreadId=OWNER, offerId=l["allocation_id"], state="ACCEPTED", phase=phase,
        acceptedAtEpoch=l["accepted_epoch"], workDeadlineEpoch=l["work_stop_epoch"],
        fullDeadlineEpoch=l["full_stop_epoch"], kind="copied-native", processCap=l["max_processes"],
        rssCapBytes=l["max_rss_bytes"], remainingNewUpperBytes=l["new_cap_bytes"],
        controller=controller, guardControllers=guards, publishedAtEpoch=time.time(),
        sourceCommit=l["source_commit"], sourceHashes=l["source_hashes"],
        receiptPaths={"accept": str(Path(l["output"])/"accepted.json"), "full": str(Path(l["output"])/"full.json")})
    atomic_json(OWN_RECEIPT, value)

def descendants(rows, roots):
    tree = set(roots) & rows.keys()
    while True:
        more = {pid for pid, row in rows.items() if row["parent"] in tree}
        if more <= tree:
            return tree
        tree |= more

def global_sample(l, own_pids, previous_claims):
    """Each guard independently reads receipts and PS; absence never releases."""
    interface = json.loads((RESOURCE / "resource-receipt-interface.json").read_bytes())
    policy = interface["limits"]
    allocation = json.loads((RESOURCE / "resource-allocation-current.json").read_bytes())
    legacy = json.loads((RESOURCE / "resource-membership-current.json").read_bytes())["entries"]
    records = {}
    for path in (RESOURCE / "owner-resource-receipts").glob("*.json"):
        require(path.stat().st_size < 4*1024**2, "bounded-resource-receipt")
        value = json.loads(path.read_bytes())
        require(path.stem == value["ownerThreadId"], "resource-owner-binding")
        records[path.stem] = value
    active = {"ACCEPTED", "ACCEPTED_RUNNING", "ACCEPTED_GUARDS_STARTING", "ACCEPTED_BOOTSTRAP", "ACCEPTED_INNER_CENSUS", "ACTUAL_ACCEPT"}
    require(all(v.get("state") in active | {"FULL", "DECLINED"} for v in records.values()), "unknown-current-resource-state")
    claims = {(o, v["offerId"], v.get("acceptedAtEpoch")): v for o, v in records.items()
              if v.get("state") in active and v.get("kind") != "ordinary"}
    for old in legacy + list(previous_claims.values()):
        if old.get("kind") == "ordinary" or old.get("state") not in active:
            require(old.get("kind") == "ordinary" or old.get("state") in active or old.get("state") in ("FULL", "DECLINED")
                    or old.get("state", "").startswith("LITERAL_FULL"), "unknown-legacy-state")
            continue
        owner = old.get("ownerThreadId", old.get("owner"))
        key = owner, old.get("offerId", old.get("cohort")), old.get("acceptedAtEpoch", old.get("actualAccept"))
        if key in claims:
            continue
        current = records.get(owner, {})
        closed = current.get("closedOffers", []) + current.get("closedReceipts", [])
        if current.get("state") == "FULL":
            closed.append(current)
        released = False
        for full in closed:
            if (full.get("ownerThreadId", owner), full.get("offerId"), full.get("acceptedAtEpoch")) != key:
                continue
            path = full.get("fullPath", full.get("immutableFULLpath", full.get("receiptPaths", {}).get("full")))
            sha = full.get("fullSHA", full.get("fullSha256", full.get("immutableFULLsha256", full.get("actualFullSHA256"))))
            resource = full.get("resources")
            zero = (type(resource) is int and resource == 0) or full.get("resourcesReleased") is True
            require(path and sha and zero and full.get("actualFullEpoch") and digest(path) == sha, "legacy-exact-full-binding")
            # The responsible-owner exact tuple attests JSON or JSONL contents;
            # no universal last-line, timestamp-proximity or later-offer shortcut.
            released = True
        if not released:
            claims[key] = dict(old, ownerThreadId=owner, offerId=key[1],
                rssCapBytes=old.get("rssCapBytes", int(old.get("rssCapGiB", 0)*1024**3)))
    values = list(claims.values())
    previous_claims.clear()
    previous_claims.update(claims)
    require(len(values) <= policy["global"] and sum(v["kind"] in ("source", "copied-native") for v in values) <= policy["source"]
            and sum(v["kind"] == "browser" for v in values) <= policy["browser"]
            and sum(v["kind"] == "target" for v in values) <= policy["warmRustTarget"], "global-class-reservations")
    reserved_pids = sum(v["processCap"] for v in values)
    reserved_rss = sum(v["rssCapBytes"] for v in values)
    rows, covered = process_table(), set()
    for v in values:
        roots = set()
        for control in [v.get("controller")] + v.get("guardControllers", []):
            if not control:
                continue
            pid = control["pid"]
            if pid not in rows or rows[pid]["birth"] != " ".join(control["birth"].split()):
                continue  # Retain reservation even when exact controller absent.
            script = control.get("script")
            if script and not Path(script).is_absolute():
                script = str(Path(control.get("cwd", control.get("workingDirectory", "/"))) / script)
            require(script and script in rows[pid]["command"], "peer-actual-controller-script")
            require(Path(control["executable"]).resolve() == Path(rows[pid]["executable"]).resolve(), "peer-actual-controller-image")
            roots.add(pid)
        tree = descendants(rows, roots)
        if v["ownerThreadId"] == OWNER:
            tree |= set(own_pids) & rows.keys()
        require(len(tree) <= v["processCap"] and sum(rows[p]["rss"] for p in tree) <= v["rssCapBytes"], "peer-actual-resource-cap")
        covered |= tree
    ordinary = set()
    for v in records.values():
        controls = ([v.get("controller")] + v.get("guardControllers", [])) if v.get("kind") == "ordinary" else []
        observation = v.get("ordinaryObservation")
        if observation:
            controls.append(dict(pid=observation.get("actualpid", observation.get("pid")), birth=observation["birth"]))
        for control in controls:
            if not control:
                continue
            pid = control["pid"]
            if pid not in rows or rows[pid]["birth"] != " ".join(control["birth"].split()):
                continue
            if control.get("script"):
                require(control["script"] in rows[pid]["command"], "ordinary-script-binding")
            tree = descendants(rows, {pid}) - covered
            for p in tree:
                name = Path(rows[p]["executable"]).name.lower()
                require(name not in ("rustc", "cargo", "node", "postgres", "redis-server") and "Google Chrome --" not in rows[p]["command"], "ordinary-runtime-payload")
            ordinary |= tree
    infrastructure = []
    infrastructure.extend(interface.get("verifiedInfrastructureObservations", []))
    for name in ("verifiedToolInfrastructureObservations", "verifiedDesktopKernelWorkerObservations"):
        infrastructure.extend(interface.get(name, {}).get("exactIdentities", []))
    expanded_infrastructure = []
    for value in infrastructure:
        if "pid" in value:
            expanded_infrastructure.append(value)
        elif len(value.get("pids", [])) == len(value.get("actualPrograms", [])):
            expanded_infrastructure.extend(dict(value, pid=pid, actualProgram=program)
                for pid, program in zip(value.get("pids", []), value.get("actualPrograms", [])))
    infra_pids = set()
    for v in expanded_infrastructure:
        pid = v["pid"]
        if pid not in rows or rows[pid]["birth"] != " ".join(v["birth"].split()) or rows[pid]["parent"] != v["ppid"]:
            continue
        if v.get("argv"):
            if rows[pid]["command"] != v["argv"]:
                continue
        else:
            if v.get("executable") != rows[pid]["executable"]:
                continue
            if v.get("actualProgram") and v["actualProgram"] not in rows[pid]["command"].split():
                continue
        infra_pids.add(pid)
    covered |= ordinary
    unknown = []
    for pid, row in rows.items():
        if pid in covered or pid in infra_pids:
            continue
        name, command = Path(row["executable"]).name.lower(), row["command"]
        scoped = any(s in command for s in ("/mcr-worktrees/", "/mcr-build/", "/private/tmp/mcr-", "/tmp/mcr-", "/T/mcr-", "/mcr-evidence/"))
        runtime = name in ("node", "cargo", "rustc", "postgres", "redis-server") or name.startswith("python") or "Google Chrome --" in command
        inline = name.startswith("python") and (command.endswith(" -") or " -c " in command)
        if scoped and runtime and not inline:
            unknown.append(pid)
    require(not unknown, "unresolved-resource-root-pids:" + ",".join(map(str, unknown)))
    ordinary_rss = sum(rows[p]["rss"] for p in ordinary)
    require(reserved_pids + len(ordinary) <= policy["sumPIDCaps"] and reserved_rss + ordinary_rss <= policy["sumRSSBytes"], "global-reservation-plus-ordinary-cap")
    require(len(covered) <= policy["sumPIDCaps"] and sum(rows[p]["rss"] for p in covered) <= policy["sumRSSBytes"], "global-physical-cap")
    all_future = max(allocation["conservativeApplicableFutureUpperBytes"], l["new_cap_bytes"] + l["other_remaining_bytes"])
    require(shutil.disk_usage(l["output"]).free - all_future >= 9*1024**3, "fresh-all-applicable-future-floor")
    return {"epoch": time.time(), "reserved_pids": reserved_pids, "reserved_rss": reserved_rss,
            "ordinary_pids": sorted(ordinary), "actual_pids": sorted(covered), "infra_pids": sorted(infra_pids),
            "all_future_bytes": all_future, "owners": sorted({v["ownerThreadId"] for v in values})}

def import_cohort(root):
    root = Path(root)
    sys.path.insert(0, str(root / "backend/scripts/mcr"))
    return importlib.import_module("browser_auth_smoke")

class RuntimeSQL:
    """A new explicit runtime cohort; old SQL acceptance is never inherited."""
    def __init__(self, config):
        self.c, self.checks = config, []
        self.inventory_path = Path(config["runtime_inventory_path"])
        require(digest(self.inventory_path) == config["runtime_inventory_sha256"], "runtime-inventory-pin")
        self.inventory = load_private(self.inventory_path)
        self.snapshot_root = None
        require(config["runtime_cohort_id"] == "BE03-689-BINARIES-CURRENT-RUNTIME-SQL-01", "explicit-new-runtime-cohort")
        require(config["runtime_read_until_epoch"] <= 1793851200, "bounded-runtime-read-custody")
        require(set(self.inventory["services"]) == {"api-gateway", "auth-service", "profile-service", "friend-service", "post-service", "notification-service"}, "all-six-runtime-bindings")
        for name, item in self.inventory["services"].items():
            changes = item["changes"]
            require(not changes["removed"] and not changes["modified"] and changes["added"] ==
                    (["0031_concert_feed_owner.sql"] if name == "post-service" else []), "bounded-runtime-sql-delta:"+name)
            if name == "post-service":
                require(item["files"]["0031_concert_feed_owner.sql"]["sha256"] ==
                        "82b7a25170d78281158e2a889614e08f343e57627846eee7310de97aefd386ac", "exact-selected-post31")
        self.manifests = {}
        self.check("initial-runtime-readset")
    def check(self, phase):
        require(time.time() < self.c["runtime_read_until_epoch"], "runtime-read-lease-ended")
        require(digest(self.inventory_path) == self.c["runtime_inventory_sha256"], "runtime-inventory-unchanged")
        for name, item in self.inventory["services"].items():
            directory = Path(item["runtime_directory"])
            info = directory.lstat()
            require(stat.S_ISDIR(info.st_mode) and not directory.is_symlink() and
                    [info.st_dev, info.st_ino] == item["directory_identity"], "runtime-directory-identity:"+name)
            files = {p.name: p for p in directory.glob("*.sql")}
            require(set(files) == set(item["files"]), "runtime-sql-file-set:"+name)
            versions = {}
            for filename, path in files.items():
                info = path.lstat()
                require(stat.S_ISREG(info.st_mode) and not path.is_symlink(), "runtime-sql-regular-file:"+name)
                content = path.read_bytes()
                require(hashlib.sha256(content).hexdigest() == item["files"][filename]["sha256"] and
                        hashlib.sha384(content).hexdigest() == item["files"][filename]["sha384"], "runtime-sql-byte-pin:"+name)
                if self.snapshot_root is not None:
                    require(digest(self.snapshot_root/name/filename) == item["files"][filename]["sha256"], "retained-runtime-snapshot-pin:"+name)
                if not filename.endswith(".down.sql"):
                    match = re.fullmatch(r"([0-9]+)_.+\.sql", filename)
                    require(match and int(match[1]) > 0 and int(match[1]) not in versions, "runtime-migration-versions:"+name)
                    versions[int(match[1])] = hashlib.sha384(content).hexdigest()
            require(hashlib.sha256(json.dumps(versions, sort_keys=True).encode()).hexdigest() == item["runtime_manifest_sha256"], "runtime-manifest-pin:"+name)
            self.manifests[name] = versions
        self.checks.append({"phase": phase, "epoch": time.time(), "all_six_match": True})
    def snapshot(self, output):
        require(self.c.get("runtime_materialization_allowed") is True, "allocated-runtime-snapshot-required")
        self.check("before-runtime-snapshot")
        root = Path(output)/"runtime-sql"
        root.mkdir(mode=0o700)
        for name, item in self.inventory["services"].items():
            target = root/name
            target.mkdir(mode=0o700)
            require(digest(item["binary"]) == item["binary_sha256"] and
                    str(Path(item["runtime_directory"]).parent).encode() in Path(item["binary"]).read_bytes(), "runtime-binary-build-path:"+name)
            for filename, pin in item["files"].items():
                content = (Path(item["runtime_directory"])/filename).read_bytes()
                require(hashlib.sha256(content).hexdigest() == pin["sha256"], "snapshot-source-pin:"+name)
                fd = os.open(target/filename, os.O_WRONLY|os.O_CREAT|os.O_EXCL, 0o400)
                with os.fdopen(fd, "wb") as stream:
                    stream.write(content)
            target.chmod(0o500)
        root.chmod(0o500)
        self.snapshot_root = root
        self.check("after-runtime-snapshot")
        return str(root)

class OwnedCase:
    """Only the parent's fresh marker/inode-bound mcr_auth; SQL goes via stdin."""
    def __init__(self, context):
        self.c = load_private(context)
        self.root = Path(self.c["root"])
        self.report = {"statuses": {}, "result": "FAIL", "secrets_recorded": False}
        self.runtime = RuntimeSQL(self.c)
        self.runtime.check("before-case")
        self.assert_owned()
        self.subject = import_cohort(self.c["cohort_root"])
        self.browser = self.subject.Browser(self.c["gateway_url"], self.report)
        self.email = "be03-" + secrets.token_hex(12) + "@example.test"
        self.password = secrets.token_urlsafe(24)
        self.user = None
        self.report["fixture"] = "fresh-own-database; no production clock configuration change"
        live = self.sql("select current_database()='mcr_auth' and "
                        "current_setting('data_directory')=" + self.literal(str(self.root / "data")) +
                        " and pg_postmaster_start_time()::text=" + self.literal(self.c["pg_start"]) +
                        " and (select system_identifier::text from pg_control_system())=" +
                        self.literal(self.c["system_identifier"]) + ";")
        require(live == "t", "actual-owned-database-identity")
    def assert_owned(self):
        c = self.c
        require(time.time() < c["work_stop_epoch"], "work-deadline")
        info = self.root.lstat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid()
                and [info.st_dev, info.st_ino] == c["root_identity"], "cluster-identity")
        marker = load_private(self.root / "mcr-owner.json")
        require(marker == {"token": c["token"], "uid": os.getuid()}, "cluster-marker")
        require(not (self.root / "data").is_symlink() and
                not (self.root / "socket").is_symlink(), "cluster-paths")
        mail = self.root / "mail"
        info = mail.lstat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid()
                and info.st_mode & 0o077 == 0, "private-mail-directory")
    @staticmethod
    def literal(value):
        return "'" + str(value).replace("'", "''") + "'"
    def sql(self, query):
        self.assert_owned()
        done = subprocess.run([self.c["psql"], "-X", "--no-password", "-A", "-t",
            "-h", str(self.root / "socket"), "-p", "5432", "-U", "mcr_owner", "-d", "mcr_auth",
            "-v", "ON_ERROR_STOP=1"], input=query, text=True, capture_output=True,
            env={"PATH": "/usr/bin:/bin", "LC_ALL": "C", "PGPASSWORD": self.c["password"],
                 "PGPASSFILE": "/dev/null", "PGCONNECT_TIMEOUT": "5"}, timeout=8)
        require(done.returncode == 0, "owned-sql-failed")
        return done.stdout.strip()
    def request(self, label, path, payload, expected=200):
        self.assert_owned()
        return self.browser.request(label, path, payload, expected=expected)[1]
    def signup(self):
        value = self.request("setup-signup", "/api/auth/email/signup",
                             {"email": self.email, "password": self.password})
        self.user = str(UUID(value["userId"]))
        require(self.sql("select count(*) from users where id=" + self.literal(self.user) +
                         " and primary_email=" + self.literal(self.email) +
                         " and status='PENDING_VERIFICATION' and deleted_at is null;") == "1",
                "fresh-unverified-user")
        require(self.unverified(), "live-unverified-email-identity")
    def unverified(self):
        return self.sql("select count(*) from user_identities where user_id=" + self.literal(self.user) +
                        " and provider_type='EMAIL' and email_verified=false and deleted_at is null;") == "1"
    def mail(self, excluded=()):
        until = min(time.time() + 20, self.c["work_stop_epoch"])
        while time.time() < until:
            self.assert_owned()
            for path in (self.root / "mail").glob("*.json"):
                value = load_private(path)
                if (value["kind"] == "VERIFY_EMAIL" and
                    value["payload"]["email"] == self.email and value["delivery_id"] not in excluded):
                    require(str(UUID(value["delivery_id"])) == value["delivery_id"], "mail-identity")
                    code = value["payload"]["verification_code"]
                    require(isinstance(code, str) and re.fullmatch(r"[0-9]{6}", code), "mail-code-shape")
                    return value
            time.sleep(.1)
        raise Refusal("actual-mail-delivery-timeout")
    def state(self):
        u = self.literal(self.user)
        return self.sql("select json_build_object('verification_ids',(select coalesce(json_agg(id order by id),'[]'::json) from email_verifications where user_id=" + u +
            "),'outbox_ids',(select coalesce(json_agg(id order by id),'[]'::json) from auth_mail_outbox where user_id=" + u +
            " and kind='VERIFY_EMAIL'),'resend_audits',(select count(*) from auth_audit_events where subject_user_id=" + u +
            " and action='EMAIL_VERIFICATION_RESENT'))::text;")
    def expired(self):
        self.signup()
        delivered = self.mail()
        u = self.literal(self.user)
        require(self.sql("with changed as (update email_verifications set expires_at=clock_timestamp()-interval '1 second' where user_id=" + u +
                         " returning id) select count(*) from changed;") == "1", "expiry-fixture-exact-one")
        self.request("expired-email-rejected", "/api/auth/email/verify",
                     {"email": self.email, "code": delivered["payload"]["verification_code"]}, 400)
        require(self.unverified() and self.sql("select status from users where id=" + u + ";") == "PENDING_VERIFICATION",
                "expiry-does-not-activate-or-verify")
        self.request("expired-user-cannot-login", "/api/auth/login",
                     {"email": self.email, "password": self.password}, 401)
        self.report.update(result="PASS", case="expired", expiry="CONTROLLED_OWNED_DB_TIMESTAMP",
                           account_remained_unverified=True)
    def resend(self):
        self.signup()
        original = self.mail()
        u = self.literal(self.user)
        before = self.state()
        require(self.request("initial-cooldown-neutral", "/api/auth/email/resend", {"email": self.email}, 202) == {"status": "ACCEPTED"}, "resend-neutral")
        require(self.state() == before, "initial-cooldown-no-queued-effect")
        require(self.sql("with changed as (update auth_mail_outbox set created_at=clock_timestamp()-interval '61 seconds' where user_id=" + u +
                         " and kind='VERIFY_EMAIL' returning id) select count(*) from changed;") == "1", "cooldown-fixture-exact-one")
        prior = json.loads(before)
        require(self.request("eligible-resend-neutral", "/api/auth/email/resend", {"email": self.email}, 202) == {"status": "ACCEPTED"}, "eligible-resend-neutral")
        delivery = self.mail([original["delivery_id"]])
        after = json.loads(self.state())
        require(len(after["verification_ids"]) == 1 and after["verification_ids"] != prior["verification_ids"]
                and len(after["outbox_ids"]) == len(prior["outbox_ids"]) + 1
                and delivery["delivery_id"] in after["outbox_ids"]
                and after["resend_audits"] == prior["resend_audits"] + 1, "fresh-verification-outbox-audit")
        code = delivery["payload"]["verification_code"]
        require(self.sql("select count(*) from email_verifications where user_id=" + u +
            " and code=" + self.literal(code) + " and expires_at>clock_timestamp();") == "1", "new-mail-code-matches-live-row")
        stable = self.state()
        require(self.request("repeated-resend-cooldown", "/api/auth/email/resend", {"email": self.email}, 202) == {"status": "ACCEPTED"}, "repeat-resend-neutral")
        require(self.state() == stable, "repeated-cooldown-no-queued-effect")
        self.assert_owned()
        matches = [load_private(p) for p in (self.root / "mail").glob("*.json")]
        require(sum(v["kind"] == "VERIFY_EMAIL" and v["payload"]["email"] == self.email for v in matches) == 2,
                "exact-two-actual-deliveries")
        result = self.request("resent-code-verifies", "/api/auth/email/verify", {"email": self.email, "code": code})
        require(result.get("userId") == self.user and result.get("status") == "VERIFIED", "verification-canonical-identity")
        require(self.sql("select status from users where id=" + u + ";") == "ACTIVE" and
                self.sql("select count(*) from user_identities where user_id=" + u +
                         " and provider_type='EMAIL' and email_verified=true and deleted_at is null and email=" +
                         self.literal(self.email) + ";") == "1", "verified-active-state")
        self.report.update(result="PASS", case="resend", cooldown="CONTROLLED_OWNED_OUTBOX_TIMESTAMP",
                           actual_deliveries=2, neutral_repeat_without_effect=True, canonical_identity_preserved=True)

def allocated_usage(paths):
    # Cleanup may unlink a just-enumerated PG file. Retry the entire inode union;
    # retain guard maxima and fail closed after three bounded complete attempts.
    for attempt in range(3):
        seen, total, pending = set(), 0, [str(Path(p).resolve()) for p in paths]
        try:
            while pending:
                path = pending.pop()
                info = os.lstat(path)
                key = info.st_dev, info.st_ino
                if key in seen:
                    continue
                seen.add(key)
                total += info.st_blocks * 512
                if stat.S_ISDIR(info.st_mode):
                    with os.scandir(path) as children:
                        pending.extend(e.path for e in children)
            return total
        except FileNotFoundError:
            if attempt == 2:
                raise Refusal("footprint-census-unstable") from None
            time.sleep(.2)

class Guard:
    """Independent instances in outer process and inner continuously live thread."""
    def __init__(self, lease, root_pid, output):
        self.l, self.root_pid, self.output = lease, root_pid, Path(output)
        self.observed, self.initial, self.samples = {}, None, 0
        self.global_claims = {}
        self.peaks = {"whole": lease["retained_whole_bytes"], "new": lease["prep_charge_bytes"], "pids": 0, "rss": 0}
        for key, value in lease.get("prior_peaks", {}).items():
            self.peaks[key] = max(self.peaks[key], value)
        self.minimum_free = shutil.disk_usage(self.output).free
        require(lease["state"] == "ACTUALLY_ACCEPTED" and lease["task"] == "BE03-ORIGINAL-EMAIL-12CC-01", "finite-native-allocation")
        require(lease["target_access"] is False and lease["cohort_baseline"] == BASELINE, "scope")
        require(self.minimum_free >= lease["start_floor_bytes"] >= 11*1024**3 and
                self.minimum_free-lease["new_cap_bytes"]-lease["other_remaining_bytes"] >= 9*1024**3, "admission-floors")
        require(time.time() < lease["work_stop_epoch"] < lease["full_stop_epoch"], "fixed-clocks")
        for name, sha in lease["source_hashes"].items():
            require(digest(name) == sha, "source-pin")
        require(digest(sys.executable) == lease["python_sha256"], "python-pin")
    @staticmethod
    def rows():
        result = {}
        for line in subprocess.check_output(["/bin/ps", "-axo", "pid=,ppid=,rss=,lstart=,comm="], text=True, timeout=4).splitlines():
            fields = line.split(None, 8)
            if len(fields) == 9:
                result[int(fields[0])] = (int(fields[1]), int(fields[2])*1024, tuple(fields[3:8]), fields[8])
        return result
    def owned(self, rows):
        owned = {self.root_pid}
        owned.update(pid for pid, identity in self.observed.items() if pid in rows and rows[pid][2:] == identity)
        marker = self.output / "cluster-owner.json"
        if marker.exists():
            c = load_private(marker)
            root = Path(c["root"])
            if root.exists():
                info = root.lstat()
                require([info.st_dev, info.st_ino] == c["root_identity"] and info.st_uid == os.getuid() and
                        load_private(root / "mcr-owner.json") == {"token": c["token"], "uid": os.getuid()}, "observer-cluster-identity")
                pidfile = root / "data/postmaster.pid"
                if pidfile.exists():
                    lines = pidfile.read_text().splitlines()
                    require(lines[1] == str(root / "data"), "observer-postmaster-data")
                    pid = int(lines[0])
                    if pid in rows:
                        require(Path(rows[pid][3]).name == "postgres", "observer-postmaster-process")
                        owned.add(pid)
        while True:
            descendants = {pid for pid, value in rows.items() if value[0] in owned}
            if descendants <= owned:
                break
            owned |= descendants
        owned &= rows.keys()
        for pid in owned:
            if pid != self.root_pid:
                self.observed[pid] = rows[pid][2:]
        return owned
    def sample(self, enforce_time=True):
        l = self.l
        rows = self.rows()
        owned = self.owned(rows)
        global_resources = global_sample(l, owned, self.global_claims)
        require(self.root_pid in rows, "outer-guard-live")
        rss = sum(rows[pid][1] for pid in owned) + 32*1024**2
        # PS already enumerates both controllers and its live census children.
        # Enforce the allocated distinct actual PIDs; do not double-charge two
        # imaginary census processes on top of that complete physical snapshot.
        count = len(owned)
        outside = allocated_usage(l["outside_paths"])
        whole = max(self.peaks["whole"], l["saved_full_target_bytes"] + outside)
        new = l["prep_charge_bytes"] + allocated_usage([self.output]) + max(0, allocated_usage(l["git_paths"]) - l["git_baseline_bytes"])
        free = shutil.disk_usage(self.output).free
        self.minimum_free = min(self.minimum_free, free)
        for key, value in (("whole", whole), ("new", new), ("pids", count), ("rss", rss)):
            self.peaks[key] = max(self.peaks[key], value)
        self.samples += 1
        current = {"epoch": time.time(), "pids": sorted(owned), "whole": whole, "new": new, "rss": rss, "count": count, "free": free,
                   "global": global_resources}
        if self.initial is None:
            self.initial = current
        require(whole <= l["whole_cap_bytes"] and new <= l["new_cap_bytes"], "bytes-cap")
        require(count <= l["max_processes"] and rss <= l["max_rss_bytes"], "process-rss-cap")
        require(free >= 8*1024**3, "live-free-floor")
        require(not enforce_time or time.time() < l["work_stop_epoch"], "work-deadline")
        return current
    def report(self):
        return {"initial_complete_sample": self.initial, "samples": self.samples, "peaks": self.peaks, "minimum_free_bytes": self.minimum_free}

def freeze_cohort(l):
    root = Path(l["cohort_root"])
    env = {"PATH": "/usr/bin:/bin", "GIT_OPTIONAL_LOCKS": "0"}
    require(subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, env=env, text=True).strip() == COHORT_HEAD, "retained-cohort-head")
    require(not subprocess.check_output(["git", "status", "--porcelain"], cwd=root, env=env), "retained-cohort-clean")
    receipt_path = root / "docs/mcr/evidence/browser-auth/native-email.json"
    # The worker checkout may predate the integrated receipt. The immutable
    # committed receipt is separately granted and source/byte-bound by the lease.
    receipt_path = Path(l["receipt_path"])
    require(digest(receipt_path) == RECEIPT, "accepted-email-receipt")
    receipt = json.loads(receipt_path.read_bytes())
    for name, sha in {**receipt["source_files_sha256"], **receipt["contract_inputs_sha256"]}.items():
        require(digest(root / name) == sha, "retained-production-or-contract-pin")
    for name, sha in l["helper_hashes"].items():
        require(digest(root / name) == sha, "retained-helper-pin")
    for service in receipt["owned_stack"]["services"]:
        require(digest(Path(l["bin_dir"]) / service["service"]) == service["binary_sha256"], "retained-binary-pin")
    module = import_cohort(root)
    for service in receipt["owned_stack"]["services"]:
        manifest = module.dev.migration.migration_manifest(root / "backend", service["service"])
        value = hashlib.sha256(json.dumps(manifest, sort_keys=True).encode()).hexdigest()
        require(value == service["migration_manifest_sha256"], "retained-migration-manifest")
    require(l.get("runtime_cohort_id") == "BE03-689-BINARIES-CURRENT-RUNTIME-SQL-01", "explicit-current-runtime-binding-required")
    return module

def run_inner(lease_path):
    l = load_private(lease_path)
    out = Path(l["output"])
    private_json(out / "inner-bootstrap.json", actual_identity(os.getpid(), out))
    bootstrap_stop = min(l["work_stop_epoch"], time.time()+90)
    while not (out / "outer-ready.json").exists():
        require(time.time() < bootstrap_stop, "outer-initial-timeout")
        time.sleep(.1)
    ready = load_private(out / "outer-ready.json")
    require(ready["allocation_id"] == l["allocation_id"], "outer-allocation-binding")
    os.kill(ready["pid"], 0)
    g = Guard(l, ready["pid"], out)
    stopped, initial = threading.Event(), threading.Event()
    failures, child, stack, runtime_sql = [], None, None, None
    report = {"result": "FAIL", "task": l["task"], "baseline": BASELINE, "old53_replayed": False,
              "Chrome12_replayed": False, "OAuth_mounted_state": "OPEN_NOT_RUN", "secrets_recorded": False}
    def watch():
        try:
            g.sample()
            initial.set()
            while not stopped.wait(.25):
                g.sample()
        except BaseException as error:
            failures.append(type(error).__name__ + (":" + str(error) if isinstance(error, Refusal) else ""))
            initial.set()
            os.kill(os.getpid(), signal.SIGINT)
    watcher = threading.Thread(target=watch, name="be03-independent-inner-guard")
    old_signals = {}
    def interrupted(*_):
        raise KeyboardInterrupt
    try:
        for sig in (signal.SIGTERM, signal.SIGINT):
            old_signals[sig] = signal.signal(sig, interrupted)
        watcher.start()
        require(initial.wait(10) and not failures and watcher.is_alive(), "inner-live-initial")
        report["before_any_workload_exe"] = {"outer": ready["initial"], "inner": g.initial, "epoch": time.time()}
        subject = freeze_cohort(l)
        runtime_sql = RuntimeSQL(l)
        report["runtime_cohort_id"] = l["runtime_cohort_id"]
        report["old_sql_acceptance_inherited"] = False
        report["runtime_inventory_sha256"] = l["runtime_inventory_sha256"]
        report["runtime_snapshot"] = runtime_sql.snapshot(out)
        require(digest(l["rustc"]) == l["rustc_sha256"], "compiler-pin")
        if l.get("prior_test_binary"):
            binary = Path(l["prior_test_binary"])
            require(digest(binary) == l["prior_test_binary_sha256"] and digest(l["rust_source"]) == l["prior_rust_source_sha256"], "same-offer-single-build-reuse")
        else:
            binary = out / "be03_original_completion"
            with (out / "compile.stdout").open("wb") as stdout, (out / "compile.stderr").open("wb") as stderr:
                child = subprocess.Popen([l["rustc"], "--test", "--edition=2021", "-C", "debuginfo=0", l["rust_source"], "-o", str(binary)],
                    stdout=stdout, stderr=stderr, env={"PATH": "/usr/bin:/bin", "TMPDIR": str(out), "LC_ALL": "C"})
                code = child.wait(timeout=max(.1, l["work_stop_epoch"]-time.time()))
            require(code == 0, "standalone-native-test-build")
        report["test_binary_sha256"] = digest(binary)
        dev = subject.dev
        old_preflight, old_execute, old_child = dev.migration.preflight, dev.migration.execute, dev.runtime.Child
        def bound_preflight(*args, **kwargs):
            binaries, tools, _ = old_preflight(*args, **kwargs)
            runtime_sql.check("before-selected-manifest-preflight")
            return binaries, tools, {name: dict(manifest) for name, manifest in runtime_sql.manifests.items()}
        def bound_execute(argv, *args, **kwargs):
            name = Path(argv[0]).name
            boundary = name in runtime_sql.manifests and len(argv) > 1 and argv[1] in ("--migrate", "--verify-migrations")
            if boundary:
                runtime_sql.check("before-cli:"+name+":"+argv[1])
            try:
                return old_execute(argv, *args, **kwargs)
            finally:
                if boundary:
                    runtime_sql.check("after-cli:"+name+":"+argv[1])
        class BoundChild(old_child):
            def __init__(self, cluster, name, *args, **kwargs):
                self.bound_service = name if name in runtime_sql.manifests else None
                if self.bound_service:
                    runtime_sql.check("before-startup:"+name)
                super().__init__(cluster, name, *args, **kwargs)
            def wait_listeners(self, *args, **kwargs):
                try:
                    return super().wait_listeners(*args, **kwargs)
                finally:
                    if self.bound_service:
                        runtime_sql.check("after-startup:"+self.bound_service)
        dev.migration.preflight, dev.migration.execute, dev.runtime.Child = bound_preflight, bound_execute, BoundChild
        original = dev.migration.Cluster
        class LocalCluster(original):
            def __init__(self, tools, root=None):
                require(root is None, "new-cluster-only")
                super().__init__(tools, out / "cluster")
                private_json(out / "cluster-owner.json", {"root": str(self.root), "root_identity": list(self.identity), "token": self.token})
        dev.migration.Cluster = LocalCluster
        original_ledger = dev.migration.check_ledger
        def diagnosed_ledger(cluster, database, expected):
            try:
                return original_ledger(cluster, database, expected)
            except dev.migration.SmokeError:
                cluster.assert_owned()
                rows = cluster.sql(database, "select version, success, encode(checksum,'hex') from public._sqlx_migrations order by version;")
                actual = {int(v): {"success": ok == "t", "checksum": sha} for v, ok, sha in (line.split("|") for line in rows.splitlines())}
                report["migration_mismatch"] = {"database": database, "expected_count": len(expected), "actual_count": len(actual),
                    "missing_versions": sorted(set(expected)-set(actual)), "extra_versions": sorted(set(actual)-set(expected)),
                    "different_versions": sorted(v for v in set(expected)&set(actual) if expected[v] != actual[v]["checksum"] or not actual[v]["success"])}
                raise
        dev.migration.check_ledger = diagnosed_ledger
        stack = dev.DevStack(Path(l["bin_dir"]), Path(l["pg_bin"]), Path(l["redis"]), Path(l["openssl"]))
        runtime_sql.check("before-stack-startup")
        descriptor = stack.start()
        runtime_sql.check("after-stack-startup")
        stack.cluster.assert_owned()
        context = {"root": str(stack.cluster.root), "root_identity": list(stack.cluster.identity), "token": stack.cluster.token,
            "password": stack.cluster.password, "psql": str(stack.cluster.tools["psql"]), "cohort_root": l["cohort_root"],
            "gateway_url": descriptor["gateway_url"], "work_stop_epoch": l["work_stop_epoch"],
            "pg_start": stack.cluster.sql("mcr_auth", "select pg_postmaster_start_time()::text;"),
            "system_identifier": stack.cluster.sql("mcr_auth", "select system_identifier::text from pg_control_system();"),
            "runner_sha256": digest(__file__), "case_output": str(out),
            **{key: l[key] for key in ("runtime_inventory_path", "runtime_inventory_sha256", "runtime_cohort_id", "runtime_read_until_epoch")}}
        context_path = out / "context.json"
        private_json(context_path, context)
        env = {"PATH": "/usr/bin:/bin", "MCR_BE03_CONTEXT": str(context_path), "MCR_BE03_RUNNER": str(Path(__file__).resolve()),
               "MCR_BE03_PYTHON": sys.executable, "PYTHONDONTWRITEBYTECODE": "1", "TMPDIR": str(out)}
        with (out / "tests.stdout").open("wb") as stdout, (out / "tests.stderr").open("wb") as stderr:
            child = subprocess.Popen([str(binary), "--ignored", "--test-threads=1"], stdout=stdout, stderr=stderr, env=env)
            code = child.wait(timeout=max(.1, l["work_stop_epoch"]-time.time()))
        output = (out / "tests.stdout").read_text()
        require(code == 0 and sorted(re.findall(r"^test (\S+) \.\.\. ok$", output, re.M)) == sorted(CASES.values())
                and "2 passed; 0 failed; 0 ignored" in output, "exact-two-native-cases")
        report["cases"] = {case: load_private(out / ("case-" + case + ".json")) for case in CASES}
        require(all(v["result"] == "PASS" for v in report["cases"].values()), "actual-case-reports")
        runtime_sql.check("after-both-cases")
        freeze_cohort(l)
        require(not failures, "live-resource-guards")
        report["result"] = "PASS_TWO_SOURCE_BOUND_EMAIL_CASES"
    except BaseException as error:
        report["failure"] = str(error) if isinstance(error, Refusal) else type(error).__name__
        if type(error).__name__ == "SmokeError" and hasattr(error, "detail"):
            report["failure_phase"] = error.detail.get("phase")
    finally:
        if child is not None and child.poll() is None:
            child.kill()
            child.wait(timeout=5)
        if stack is not None:
            try:
                runtime_sql.check("before-cleanup")
            except BaseException:
                report["runtime_before_cleanup_mismatch"] = True
                report["result"] = "FAIL"
            try:
                stack.close()
            except BaseException:
                report["cleanup_error"] = True
            report["owned_stack"] = stack.report
            if stack.report.get("cleanup") != "PASS":
                report["result"] = "FAIL"
            try:
                runtime_sql.check("after-cleanup")
            except BaseException:
                report["runtime_after_cleanup_mismatch"] = True
                report["result"] = "FAIL"
        if runtime_sql is not None:
            report["runtime_sql_checks"] = runtime_sql.checks
        stopped.set()
        if watcher.ident is not None:
            watcher.join(timeout=5)
        if failures or watcher.is_alive():
            report["result"] = "FAIL"
        report.update(guard_errors=failures, inner_guard=g.report(), ended_epoch=time.time(), inner_thread_reaped=not watcher.is_alive())
        (out / "context.json").unlink(missing_ok=True)
        private_json(out / "report.json", report)
        for sig, handler in old_signals.items():
            signal.signal(sig, handler)
    return 0 if report["result"] == "PASS_TWO_SOURCE_BOUND_EMAIL_CASES" else 1

def supervise(lease_path):
    l = load_private(lease_path)
    out = Path(l["output"])
    identity = actual_identity(os.getpid(), out)
    publish_accept(l, identity, [], "ACTUAL_OUTER_BOOTSTRAP")
    private_json(out / "accepted.json", {"lease": l, "actual_guard": identity, "published_epoch": time.time()})
    g = Guard(l, os.getpid(), out)
    child, reason = None, None
    try:
        child = subprocess.Popen([sys.executable, "-B", str(Path(__file__).resolve()), "--run", str(lease_path)], start_new_session=True,
            env={"PATH": "/usr/bin:/bin", "PYTHONDONTWRITEBYTECODE": "1", "TMPDIR": str(out)},
            stdout=(out / "runner.stdout").open("wb"), stderr=(out / "runner.stderr").open("wb"))
        bootstrap_stop = min(l["work_stop_epoch"], time.time()+30)
        while not (out / "inner-bootstrap.json").exists():
            require(child.poll() is None and time.time() < bootstrap_stop, "inner-bootstrap-timeout")
            time.sleep(.1)
        inner = load_private(out / "inner-bootstrap.json")
        require(inner["pid"] == child.pid, "actual-inner-bootstrap-binding")
        publish_accept(l, inner, [identity], "BOTH_ACTUAL_IDENTITIES_BEFORE_CENSUS")
        initial = g.sample()
        private_json(out / "outer-ready.json", {"allocation_id": l["allocation_id"], "pid": os.getpid(), "initial": initial})
        while child.poll() is None:
            try:
                g.sample()
            except BaseException as error:
                if reason is None:
                    reason = str(error) if isinstance(error, Refusal) else type(error).__name__
                    os.kill(child.pid, signal.SIGINT)
            if time.time() >= l["full_stop_epoch"]-15:
                reason = reason or "full-deadline"
                break
            time.sleep(.25)
    except BaseException as error:
        reason = str(error) if isinstance(error, Refusal) else type(error).__name__
    finally:
        if child is not None and child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=3)
        rows = g.rows()
        remaining = g.owned(rows)-{os.getpid()}
        for pid in remaining:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        time.sleep(.1)
        live = g.rows()
        remaining = {pid for pid in remaining if pid in live and live[pid][2:] == rows[pid][2:]}
        for pid in remaining:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        if remaining:
            reason = reason or "owned-cleanup-incomplete"
        report = {"allocation_id": l["allocation_id"], "exit_code": child.returncode if child else None,
                  "stop_reason": reason, "resources": g.report(), "remaining_owned_pids": sorted(remaining),
                  "cluster_root_removed": not (out / "cluster").exists(), "ended_epoch": time.time(), "target_access": False}
        private_json(out / "guardian.json", report)
    return 0 if child and child.returncode == 0 and not reason and not remaining and report["cluster_root_removed"] else 1

def main():
    p = argparse.ArgumentParser(description=__doc__)
    mode = p.add_mutually_exclusive_group(required=True)
    mode.add_argument("--supervise", type=Path)
    mode.add_argument("--run", type=Path)
    mode.add_argument("--case", choices=CASES)
    p.add_argument("--context", type=Path)
    a = p.parse_args()
    if a.supervise:
        return supervise(a.supervise)
    if a.run:
        return run_inner(a.run)
    report = {"case": a.case, "result": "FAIL", "secrets_recorded": False}
    context = load_private(a.context)
    case = None
    try:
        require(digest(__file__) == context["runner_sha256"], "case-runner-pin")
        case = OwnedCase(a.context)
        report = case.report
        getattr(case, a.case)()
    except BaseException as error:
        report["failure"] = str(error) if isinstance(error, Refusal) or type(error).__name__ == "CheckFailure" else type(error).__name__
    finally:
        if case is not None:
            try:
                case.runtime.check("after-case")
                report["runtime_sql_checks"] = case.runtime.checks
            except BaseException:
                report.update(result="FAIL", runtime_after_case_mismatch=True)
    private_json(Path(context["case_output"]) / ("case-" + a.case + ".json"), report)
    return 0 if report["result"] == "PASS" else 1

if __name__ == "__main__":
    raise SystemExit(main())
