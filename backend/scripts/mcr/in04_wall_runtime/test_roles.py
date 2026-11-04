"""Real OpenSSL signatures; simulated read-only SQL answers, never a PG cohort.

Run with the existing scripts/mcr directory on PYTHONPATH. Tests create only
private files and OpenSSL children; no initdb, psql, service or identity runs.
"""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile
import time
import unittest
from unittest.mock import patch
from uuid import uuid4

import roles


class ManagedRoleTests(unittest.TestCase):
    def setUp(self):
        self.sandbox = tempfile.TemporaryDirectory(prefix="mcr-managed-roles-test-")
        self.addCleanup(self.sandbox.cleanup)
        self.cluster = roles.migration_smoke.Cluster(
            {"psql": Path("/nonexistent/test-only-psql")}, Path(self.sandbox.name) / "owned")
        self.cluster.data.mkdir(mode=0o700)
        self.pid = self.cluster.data / "postmaster.pid"
        self.pid.write_text(f"23456\n{self.cluster.data}\n0\n5432\n{self.cluster.socket}\n")
        self.stack = object.__new__(roles.dev_stack.DevStack)
        self.stack.cluster, self.stack.children = self.cluster, {}
        tool = os.environ.get("MCR_TEST_OPENSSL")
        if not tool:
            self.fail("MCR_TEST_OPENSSL must name the explicitly allocated actual OpenSSL binary")
        self.stack.openssl_binary = Path(tool).resolve(strict=True)
        self.deadline = roles.in03_stack_runtime.WorkDeadline(time.monotonic() + 40, time.monotonic() + 50)
        self.expiry = time.time() + 30
        self.clock = datetime.now(timezone.utc)
        self.system, self.auth, self.admitted = "7462923000111000001", True, True
        self.queries, self.commands = [], []
        self.real_run = subprocess.run
        self.addCleanup(patch.stopall)
        patch.object(roles.subprocess, "run", self.run_child).start()
        self.users = [str(uuid4()) for _ in range(3)]
        self.event = str(uuid4())
        self.records = []
        for ordinal, user in enumerate(self.users, 1):
            path = self.cluster.root / f"in01-principal-{ordinal}.json"
            roles.create_private(path, json.dumps({"user_id": user, "email": f"real{ordinal}@example.test",
                "password": "PRIVATE_PASSWORD_SENTINEL", "login_path": "/api/auth/login",
                "access_token": "MEMORY_ONLY"}).encode())
            self.records.append({"user_id": user, "credentials_path": path})

    def run_child(self, args, **kwargs):
        self.commands.append((args, kwargs))
        if args[0] != str(self.cluster.tools["psql"]):
            self.assertNotIn("PGPASSWORD", kwargs["env"])
            return self.real_run(args, **kwargs)
        self.assertLessEqual(kwargs["timeout"], 2)
        self.assertEqual(kwargs["cwd"], self.cluster.root)
        database, query = kwargs["env"]["PGDATABASE"], args[-1]
        self.queries.append((database, query))
        self.assertTrue(query.startswith("SELECT "))
        if database == "postgres":
            value = {"system_identifier": self.system, "data_directory": str(self.cluster.data),
                "socket": str(self.cluster.socket), "tcp": False, "database": "postgres",
                "owner": roles.migration_smoke.OWNER,
                "clock": self.clock.isoformat(timespec="microseconds").replace("+00:00", "Z")}
            raw = json.dumps(value).encode()
        else:
            self.assertIn(database, ("mcr_auth", "mcr_concert"))
            raw = b"t" if (self.auth if database == "mcr_auth" else self.admitted) else b"f"
        return subprocess.CompletedProcess(args, 0, raw + b"\n", b"")

    def publisher(self, **changes):
        return roles.ManagedRoles(self.stack, self.deadline,
            expected_system_identifier=changes.get("system", "7462923000111000001"),
            window_end_epoch=changes.get("expiry", self.expiry))

    def grant(self, publisher, records=None, actor=None, event=None):
        return publisher.publish_operator(event or self.event,
            self.records if records is None else records, actor or self.users[2])

    def test_initial_publication_is_private_empty_and_environment_is_public_only(self):
        publisher = self.publisher()
        env = publisher.environment()
        self.assertEqual(set(env), {"CONCERT_OPERATIONS_MODE", "CONCERT_OPERATIONS_ISSUER_KEYS_JSON",
                                   "CONCERT_OPERATIONS_PUBLICATION_FILE"})
        self.assertEqual(env["CONCERT_OPERATIONS_MODE"], "live")
        self.assertEqual(Path(env["CONCERT_OPERATIONS_PUBLICATION_FILE"]).read_bytes(), b"[]")
        keys = json.loads(env["CONCERT_OPERATIONS_ISSUER_KEYS_JSON"])
        self.assertRegex(keys[0]["public_key_hex"], r"^[a-f0-9]{64}$")
        for path in publisher._directory.iterdir():
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE(publisher._directory.stat().st_mode), 0o700)
        self.assertNotIn("PRIVATE", repr(publisher) + json.dumps(env))

    def test_exact_signed_third_actor_server_clock_expiry_and_atomic_replay(self):
        publisher = self.publisher()
        initial_inode = publisher._publication.stat().st_ino
        receipt = self.grant(publisher)
        signed = json.loads(publisher._publication.read_bytes())[0]
        statement = signed["statement"]
        self.assertEqual(tuple(statement), roles.FIELDS)
        self.assertEqual(statement["actor_id"], self.users[2])
        self.assertEqual(statement["event_id"], self.event)
        self.assertEqual((statement["version"], statement["expected_revision"], statement["role"]),
                         (1, 0, "OPERATOR"))
        self.assertEqual(roles.timestamp(statement["issued_at"]), self.clock)
        self.assertLessEqual(roles.timestamp(statement["expires_at"]).timestamp(), self.expiry)
        self.assertRegex(signed["signature_hex"], r"^[a-f0-9]{128}$")
        self.assertEqual(receipt["applied"], "NOT_OBSERVED")
        self.assertNotEqual(initial_inode, publisher._publication.stat().st_ino)
        inode, raw = publisher._publication.stat().st_ino, publisher._publication.read_bytes()
        sign_count = sum("-sign" in command for command, _ in self.commands)
        self.assertEqual(self.grant(publisher), receipt)
        self.assertEqual(publisher._publication.stat().st_ino, inode)
        self.assertEqual(publisher._publication.read_bytes(), raw)
        self.assertEqual(sum("-sign" in command for command, _ in self.commands), sign_count)
        self.assertEqual(sum(database == "mcr_auth" for database, _ in self.queries), 6)
        self.assertEqual(sum(database == "mcr_concert" for database, _ in self.queries), 6)
        self.assertNotIn("PRIVATE_PASSWORD_SENTINEL", raw.decode())

    def test_independent_real_verifier_rejects_payload_and_signature_corruption(self):
        publisher = self.publisher()
        self.grant(publisher)
        statement = json.loads(publisher._publication.read_bytes())[0]["statement"]
        # Serialize independently using the Rust struct declaration order.
        expected = b"MCR-OPERATIONS-ASSIGNMENT-V1\x00" + json.dumps({key: statement[key] for key in (
            "version", "statement_id", "issuer", "event_id", "actor_id", "expected_revision", "role",
            "issued_at", "expires_at")}, separators=(",", ":")).encode()
        message, signature = publisher._directory / "statement.bin", publisher._directory / "signature.bin"
        self.assertEqual(message.read_bytes(), expected)
        command = [str(self.stack.openssl_binary), "pkeyutl", "-verify", "-rawin", "-pubin", "-keyform",
            "DER", "-inkey", str(publisher._public), "-sigfile", str(signature), "-in", str(message)]
        def verify():
            return self.real_run(command, capture_output=True, timeout=2,
                env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"}).returncode
        self.assertEqual(verify(), 0)
        message.write_bytes(expected.replace(b"OPERATOR", b"MODERATOR"))
        self.assertNotEqual(verify(), 0)
        message.write_bytes(expected)
        raw = signature.read_bytes()
        signature.write_bytes(bytes([raw[0] ^ 1]) + raw[1:])
        self.assertNotEqual(verify(), 0)

    def test_wrong_system_and_started_concert_refused_before_key_creation(self):
        with self.assertRaises(roles.RoleRefusal):
            self.publisher(system="7462923000111000002")
        self.assertFalse((self.cluster.root / "in04-managed-roles").exists())
        self.stack.children["concert-service"] = object()
        with self.assertRaisesRegex(roles.RoleRefusal, "before-concert"):
            self.publisher()

    def test_fractional_epoch_rounding_never_extends_original_deadline(self):
        # A fractional epoch that rounds up under float *1e6 before floor.
        for epoch in (1793824600.12345695, 1793824600.9999998, 1793824600.0000002):
            with self.subTest(epoch=epoch):
                expires = roles.bounded_expiry(epoch)
                self.assertLessEqual(expires.timestamp(), epoch)
                self.assertLess(epoch - expires.timestamp(), 0.0000011)

    def test_expiry_never_extends_root_work_deadline(self):
        for expiry in (True, float("inf"), 1e308, time.time() - 1, time.time() + 100):
            with self.subTest(expiry=expiry), self.assertRaises(roles.RoleRefusal):
                self.publisher(expiry=expiry)
        self.assertFalse((self.cluster.root / "in04-managed-roles").exists())

    def test_clock_after_signing_cannot_publish_expired_grant(self):
        publisher = self.publisher()
        real_clock = publisher._clock
        calls = []
        def clock():
            calls.append(1)
            return real_clock() if len(calls) == 1 else datetime.fromtimestamp(self.expiry + 1, timezone.utc)
        with patch.object(publisher, "_clock", clock), self.assertRaises(roles.RoleRefusal):
            self.grant(publisher)
        self.assertEqual(publisher._publication.read_bytes(), b"[]")

    def test_actual_auth_and_admission_required_before_signing(self):
        publisher = self.publisher()
        for auth, admitted in ((False, True), (True, False)):
            self.auth, self.admitted = auth, admitted
            with self.subTest(auth=auth), self.assertRaises(roles.RoleRefusal):
                self.grant(publisher)
        self.assertFalse((publisher._directory / "statement.bin").exists())
        self.assertEqual(publisher._publication.read_bytes(), b"[]")

    def test_canonical_third_distinct_principals_only(self):
        publisher = self.publisher()
        bad = [dict(row) for row in self.records]
        bad[2] = dict(bad[1])
        for records, actor, event in ((self.records, self.users[0], self.event),
                (bad, self.users[2], self.event), (self.records[:2], self.users[2], self.event),
                (self.records, self.users[2], self.event.upper())):
            with self.subTest(actor=actor, count=len(records)), self.assertRaises(roles.RoleRefusal):
                self.grant(publisher, records, actor, event)
        self.assertEqual(publisher._publication.read_bytes(), b"[]")

    def test_credentials_reject_bearer_claim_wrong_user_unknown_fields_and_public_file(self):
        publisher = self.publisher()
        path = self.records[0]["credentials_path"]
        valid = path.read_bytes()
        for change in ({"access_token": "BEARER_SECRET_SENTINEL"}, {"user_id": self.users[1]},
                       {"login_user_id": self.users[0]}):
            value = json.loads(valid)
            value.update(change)
            path.write_text(json.dumps(value))
            with self.assertRaises(roles.RoleRefusal) as failure:
                self.grant(publisher)
            self.assertNotIn("SENTINEL", str(failure.exception))
        path.write_bytes(valid)
        path.chmod(0o644)
        with self.assertRaises(roles.RoleRefusal):
            self.grant(publisher)

    def test_owner_marker_and_postmaster_replacement_refused(self):
        publisher = self.publisher()
        valid_marker = self.cluster.marker.read_bytes()
        self.cluster.marker.write_text('{}')
        with self.assertRaises(roles.RoleRefusal):
            publisher.environment()
        self.cluster.marker.write_bytes(valid_marker)
        self.pid.write_text(self.pid.read_text().replace("23456", "23457"))
        with self.assertRaises(roles.RoleRefusal):
            publisher.environment()

    def test_directory_and_stack_rebinding_refused(self):
        publisher = self.publisher()
        self.stack.cluster = object()
        with self.assertRaises(roles.RoleRefusal):
            publisher.environment()
        self.stack.cluster = self.cluster
        moved = publisher._directory.with_name("old-issuer")
        publisher._directory.rename(moved)
        publisher._directory.mkdir(mode=0o700)
        with self.assertRaises(roles.RoleRefusal):
            publisher.environment()

    def test_publication_symlink_hardlink_and_tampering_refused(self):
        publisher = self.publisher()
        path = publisher._publication
        backup = publisher._directory / "old-publication"
        path.rename(backup)
        path.symlink_to(backup)
        with self.assertRaises(roles.RoleRefusal):
            publisher.environment()
        path.unlink()
        os.link(backup, path)
        with self.assertRaises(roles.RoleRefusal):
            publisher.environment()
        path.unlink()
        backup.rename(path)
        path.write_bytes(b"[{}]")
        with self.assertRaises(roles.RoleRefusal):
            self.grant(publisher)

    def test_replaced_private_key_and_replay_publication_refused(self):
        publisher = self.publisher()
        valid = publisher._key.read_bytes()
        publisher._key.write_bytes(valid[:-1] + bytes([valid[-1] ^ 1]))
        with self.assertRaises(roles.RoleRefusal):
            self.grant(publisher)
        publisher._key.write_bytes(valid)
        self.grant(publisher)
        with self.assertRaises(roles.RoleRefusal):
            self.grant(publisher, event=str(uuid4()))
        publisher._publication.write_bytes(b"[]")
        with self.assertRaises(roles.RoleRefusal):
            self.grant(publisher)

    def test_deadline_refuses_all_work_after_expiry(self):
        publisher = self.publisher()
        with patch.object(roles.time, "monotonic", return_value=self.deadline.work_monotonic + 1):
            with self.assertRaises(roles.in03_stack_runtime.InputRefusal):
                self.grant(publisher)

    def test_statement_wire_policy_rejects_reordering_revision_and_non_utc(self):
        publisher = self.publisher()
        self.grant(publisher)
        valid = json.loads(publisher._publication.read_bytes())[0]["statement"]
        for statement in (dict(reversed(tuple(valid.items()))), dict(valid, expected_revision=True),
                dict(valid, issued_at=valid["issued_at"].replace("Z", "+00:00")), dict(valid, role=None)):
            with self.assertRaises(roles.RoleRefusal):
                roles.statement_bytes(statement)


if __name__ == "__main__":
    unittest.main()
