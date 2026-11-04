#!/usr/bin/env python3
"""Pure custody/configuration/failure tests. Never starts a native service or PG."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import in03_stack_runtime as stack


class Inputs(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="mcr-in03-pure-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.revision = "a" * 40

    def file(self, name, value=b"test executable", mode=0o700):
        path = self.root / name
        path.write_bytes(value)
        path.chmod(mode)
        return path

    def pin(self, name):
        path = self.file(name)
        return stack.BinaryPin(path, hashlib.sha256(path.read_bytes()).hexdigest(), self.revision)

    def test_changed_binary_and_symbolic_link_are_refused_without_values(self):
        pin = self.pin("secret-path-and-token")
        pin.path.write_bytes(b"changed executable")
        with self.assertRaisesRegex(stack.InputRefusal, "^binary-hash-mismatch$"):
            pin.validate()
        link = self.root / "link"
        link.symlink_to(pin.path)
        with self.assertRaisesRegex(stack.InputRefusal, "^explicit-binary-path-required$"):
            stack.BinaryPin(link, pin.sha256, pin.source).validate()

    def test_executable_handles_close_and_hashing_streams(self):
        pin = self.pin("binary")
        # ResourceWarning is an error when this suite is run with -W error.
        for _ in range(30):
            self.assertEqual(pin.validate()["sha256"], pin.sha256)
        pin.path.chmod(0o720)
        with self.assertRaisesRegex(stack.InputRefusal, "^writable-binary-refused$"):
            pin.validate()

    def test_exact_cohort_rejects_missing_extra_and_shared_inode(self):
        pins = {name: self.pin(name) for name in stack.SERVICES}
        self.assertEqual(set(stack.validate_binary_cohort(pins)), stack.SERVICES)
        with self.assertRaises(stack.InputRefusal):
            stack.validate_binary_cohort({k: v for k, v in pins.items() if k != "auth-service"})
        with self.assertRaises(stack.InputRefusal):
            stack.validate_binary_cohort({**pins, "extra": pins["auth-service"]})
        target = pins["friend-service"].path
        target.unlink()
        os.link(pins["auth-service"].path, target)
        with self.assertRaisesRegex(stack.InputRefusal, "^distinct-service-inodes-required$"):
            stack.validate_binary_cohort(pins)

    def test_private_input_rejects_public_link_fifo_oversize_and_replacement(self):
        path = self.file("private", b"SECRET", 0o600)
        self.assertEqual(stack.private_input(path), b"SECRET")
        path.chmod(0o644)
        with self.assertRaisesRegex(stack.InputRefusal, "^private-input-custody-required$"):
            stack.private_input(path)
        path.chmod(0o600)
        with patch.object(stack, "identity", side_effect=[(1,), (2,)]):
            with self.assertRaisesRegex(stack.InputRefusal, "^file-changed-during-read$"):
                stack.private_input(path)
        fifo = self.root / "fifo"
        os.mkfifo(fifo, 0o600)
        with self.assertRaisesRegex(stack.InputRefusal, "^regular-file-required$"):
            stack.private_input(fifo)
        huge = self.file("huge", b"x" * (1024 * 1024 + 1), 0o600)
        with self.assertRaisesRegex(stack.InputRefusal, "^private-input-too-large$"):
            stack.private_input(huge)

    def records(self):
        records = {}
        for name in stack.SERVICES:
            pin = self.pin(name)
            raw = json.dumps({"service": name, "binary_sha256": pin.sha256,
                              "source_revision": pin.source, "result": "PASS"}).encode()
            receipt = self.file(name + ".json", raw, 0o600)
            records[name] = {"path": str(pin.path), "sha256": pin.sha256, "source": pin.source,
                "qualification": "IN03_ADVANCED_COMBINED" if name in stack.ADVANCED_SERVICES else "WORLD_AUTH_FOUNDATION",
                "evidence": {"path": str(receipt), "sha256": hashlib.sha256(raw).hexdigest()}}
        return records

    def test_receipt_binds_source_service_and_binary_not_filename(self):
        records = self.records()
        pins, provenance = stack.admitted_cohort(records)
        self.assertEqual(set(pins), stack.SERVICES)
        self.assertNotIn("path", provenance["binaries"]["concert-service"])
        records["concert-service"]["qualification"] = "WORLD_AUTH_FOUNDATION"
        with self.assertRaisesRegex(stack.InputRefusal, "^binary-qualification-mismatch$"):
            stack.admitted_cohort(records)
        records["concert-service"]["qualification"] = "IN03_ADVANCED_COMBINED"
        records["concert-service"]["source"] = "b" * 40
        with self.assertRaisesRegex(stack.InputRefusal, "^binary-evidence-binding-mismatch$"):
            stack.admitted_cohort(records)

    def test_receipt_rejects_full_bootstrap_before_hashing(self):
        records = self.records()
        path = Path(records["concert-service"]["evidence"]["path"])
        value = json.loads(path.read_bytes())
        value["participant"] = {"private": "DO_NOT_HASH"}
        path.write_text(json.dumps(value))
        with patch.object(stack.hashlib, "sha256", side_effect=AssertionError("must not hash unknown receipt")):
            # Test the malformed service first so no other legitimate receipt is hashed.
            ordered = {"concert-service": records["concert-service"], **records}
            with self.assertRaisesRegex(stack.InputRefusal, "^binary-evidence-schema$"):
                stack.admitted_cohort(ordered)

    def test_closed_json_rejects_duplicate_keys_and_nonfinite(self):
        for raw in (b'{"mode":1,"mode":2}', b'{"value":NaN}', b'not-json'):
            with self.assertRaises(stack.InputRefusal):
                stack.json_object(raw)


class RuntimeBoundaries(unittest.TestCase):
    def test_original_work_cutoff_cannot_be_refreshed_by_phase_or_cleanup(self):
        deadline = stack.WorkDeadline(100, 340)
        with patch.object(stack.time, "monotonic", return_value=100):
            with self.assertRaisesRegex(stack.InputRefusal, "^allocation-work-expired$"):
                deadline.check()
            deadline.check(cleanup=True)
        with patch.object(stack.time, "monotonic", return_value=340):
            with self.assertRaisesRegex(stack.InputRefusal, "^allocation-release-expired$"):
                deadline.check(cleanup=True)
        for work, release in ((100, 100), (float("nan"), 340), (100, float("inf"))):
            with self.assertRaises(stack.InputRefusal):
                stack.WorkDeadline(work, release)

    def managed(self):
        envs = {name: {} for name in stack.SERVICES if name != "concert-service"}
        concert = {}
        urls = {role: "https://127.0.0.1:" + str(2000 + n) for n, role in
                enumerate(("auth", "friend", "post", "notification", "concert"))}
        stack.managed_features(envs, concert, urls, (("post-seed", "post-public"), ("callback-seed", "callback-public")))
        return envs, concert

    def test_feature_keys_are_independent_and_callbacks_are_directional(self):
        envs, concert = self.managed()
        keys = [v for k, v in concert.items() if k.endswith("_KEY_HEX")]
        keys += [json.loads(v)[0]["secret_hex"] for k, v in concert.items() if k.endswith("_KEYS_JSON")]
        self.assertEqual(len(keys), 7)
        self.assertEqual(len(set(keys)), len(keys))
        self.assertTrue(all(len(v) == 64 and int(v, 16) > 0 for v in keys))
        self.assertEqual(envs["post-service"]["CONCERT_POST_READ_VERIFYING_KEYS"], "post-public")
        self.assertEqual(concert["POST_CONCERT_ROOT_READ_VERIFYING_KEYS"], "callback-public")
        self.assertFalse(any("CLAIM" in key or "AGE_PROVIDER" in key for key in concert))

    def test_requested_mode_or_unencrypted_endpoint_cannot_claim_ready(self):
        envs, concert = self.managed()
        concert["CONCERT_SOCIAL_MODE"] = "off"
        with self.assertRaisesRegex(stack.InputRefusal, "^actual-managed-mode-mismatch$"):
            stack.validate_modes(envs, concert)
        concert["CONCERT_SOCIAL_MODE"] = "live"
        envs["post-service"]["POST_CONCERT_ROOT_GRPC_ENDPOINT"] = "http://127.0.0.1:4444"
        with self.assertRaisesRegex(stack.InputRefusal, "^managed-mtls-endpoint-required$"):
            stack.validate_modes(envs, concert)

    def test_functional_pools_leave_background_capacity_without_provider_overrides(self):
        envs, concert = self.managed()
        # The actual Gateway parser rejects a one-connection journal. Keep
        # independent journal/background pools while preserving mandatory modes.
        journal = int(envs["api-gateway"]["GATEWAY_JOURNAL_DB_POOL_MAX_CONNECTIONS"])
        self.assertGreaterEqual(journal, 2)
        self.assertLessEqual(journal, 64)
        limits = [int(value) for env in envs.values() for name, value in env.items()
                  if name.endswith("_DB_POOL_MAX_CONNECTIONS")]
        self.assertEqual(len(limits), 8)  # Six owners plus the two Gateway pools.
        self.assertTrue(all(value > 0 for value in limits))
        self.assertLessEqual(sum(limits), 18)
        self.assertFalse(any("CLAIM" in key or "AGE_PROVIDER" in key for key in concert))
        stack.validate_modes(envs, concert)

    def test_accepted_provider_cannot_be_fabricated(self):
        envs, concert = self.managed()
        concert["CONCERT_SOCIAL_ACCEPTED_CLAIM_ISSUER"] = "fake"
        with self.assertRaisesRegex(stack.InputRefusal, "^unqualified-provider-override$"):
            stack.validate_modes(envs, concert)

    def test_query_or_control_failure_still_closes_all_owned_resources(self):
        owned = SimpleNamespace(cluster=SimpleNamespace(started=True, sql=Mock(side_effect=RuntimeError("secret"))),
                                close=Mock(), report={"cleanup": "PASS", "process_stops": []})
        control = SimpleNamespace(server_close=Mock(side_effect=OSError("private")))
        guard = SimpleNamespace(close=Mock(return_value={"closed": True}))
        result = stack.cleanup_owned(owned, control, guard)
        owned.close.assert_called_once_with()
        guard.close.assert_called_once_with()
        self.assertEqual(result["cleanup"], "FAIL")
        self.assertEqual(result["prepared_before_cleanup"], "UNAVAILABLE")
        self.assertNotIn("secret", json.dumps(result))
        self.assertNotIn("private", json.dumps(result))

    def test_concert_actual_readiness_does_not_use_six_service_peer_table(self):
        foundation = Mock(side_effect=AssertionError("Concert has its own actual health schema"))
        value = {"ready": True, "degraded": False, "draining": False,
                 "checks": {"database": {"required": True, "status": "up"}}}
        result = stack.seven_readiness(200, json.dumps(value).encode(), "concert-service", foundation)
        self.assertTrue(result["ready"])
        value["checks"]["database"]["status"] = "down"
        with self.assertRaisesRegex(stack.InputRefusal, "^concert-ready-consistency$"):
            stack.seven_readiness(200, json.dumps(value).encode(), "concert-service", foundation)
        value.update(ready=False, degraded=True)
        self.assertFalse(stack.seven_readiness(503, json.dumps(value).encode(), "concert-service", foundation)["ready"])

    def test_auth_provenance_uses_observed_http_actor_and_event_without_body_hash(self):
        with tempfile.TemporaryDirectory(prefix="mcr-in03-auth-pure-") as directory:
            root = Path(directory)
            users = [f"f1111111-1111-4111-8111-{n:012d}" for n in range(1, 4)]
            principals = []
            for n, user in enumerate(users, 1):
                path = root / str(n)
                path.write_text(json.dumps({"user_id": user, "email": f"actor{n}@example.test",
                                            "access_token": "MEMORY_ONLY"}))
                path.chmod(0o600)
                principals.append((user, path, "RAM_ONLY_BEARER"))
            report = {"statuses": {f"principal-{n}-{step}": 200 for n in range(1, 4)
                                    for step in ("signup", "verify", "login")}}
            event = "event-bound-in-test"
            validator = Mock()
            validator.bootstrap.side_effect = users
            raw = b'{"participant":"FULL_RAM_ONLY_BOOTSTRAP"}'
            world = SimpleNamespace(public_json=Mock(return_value=raw))
            modules = {"world_capacity_support.transport": SimpleNamespace(Contracts=lambda: validator),
                       "world_capacity_support.report": SimpleNamespace(json_value=json.loads)}
            with patch.dict(sys.modules, modules):
                with patch.object(stack.hashlib, "sha256", side_effect=AssertionError("C03 must never be hashed")):
                    receipt = stack.auth_provenance(SimpleNamespace(cluster=SimpleNamespace(token="private-owner")),
                        SimpleNamespace(origin="http://127.0.0.1:5173"), report, principals, event,
                        "a" * 40, "b" * 40, stack.time.time() + 300, world)
                self.assertEqual(set(receipt["actors"]), set(stack.ROLES))
                self.assertEqual(receipt["kind"], "CANONICAL_GATEWAY_PRIVATE_HTTP_MAIL")
                self.assertEqual(receipt["actors"]["restricted"]["userId"], users[2])
                self.assertNotIn("RAM_ONLY_BEARER", json.dumps(receipt))
                self.assertNotIn("FULL_RAM_ONLY_BOOTSTRAP", json.dumps(receipt))
                validator.bootstrap.side_effect = ["wrong-user"]
                with self.assertRaisesRegex(stack.InputRefusal, "^actual-bootstrap-owner$"):
                    stack.auth_provenance(SimpleNamespace(cluster=SimpleNamespace(token="private-owner")),
                        SimpleNamespace(origin="http://127.0.0.1:5173"), report, principals, event,
                        "a" * 40, "b" * 40, stack.time.time() + 300, world)

    def test_import_and_invalid_cli_never_load_services(self):
        with patch.object(stack, "load_helpers", side_effect=AssertionError("native helper imported")):
            with patch.object(stack, "run", side_effect=AssertionError("runtime started")):
                now = stack.time.time()
                arguments = ["--inputs", "/does-not-exist", "--report", "/does-not-exist",
                    "--browser-origin", "http://127.0.0.1:5173", "--allocation-id", "expired",
                    "--allocation-work-deadline-epoch", str(now - 1),
                    "--allocation-release-deadline-epoch", str(now + 240),
                    "--whole-timeout-seconds", "600", "--startup-timeout-seconds", "180",
                    "--provision-timeout-seconds", "120", "--hold-seconds", "120",
                    "--whole-byte-limit", "1000", "--retained-carry-bytes", "0", "--max-owned-processes", "48"]
                with patch("builtins.print") as output:
                    self.assertEqual(stack.main(arguments), 2)
                self.assertEqual(output.call_args.args, ('{"result":"REFUSED","reason":"invalid-or-unavailable-stack-input"}',))


class ManagedProfileEntry(unittest.TestCase):
    def arguments(self):
        now = stack.time.time()
        return ["--inputs", "/not-opened-in-cli-test", "--report", "/not-opened-in-cli-test",
            "--browser-origin", "http://127.0.0.1:3000", "--allocation-id", "managed-test",
            "--allocation-work-deadline-epoch", str(now + 120),
            "--allocation-release-deadline-epoch", str(now + 500),
            "--whole-timeout-seconds", "600", "--startup-timeout-seconds", "180",
            "--provision-timeout-seconds", "120", "--hold-seconds", "120",
            "--whole-byte-limit", "1000", "--retained-carry-bytes", "0", "--max-owned-processes", "48"]

    def test_default_cli_preserves_legacy_run_without_profile_argument(self):
        with patch.object(stack, "run", return_value=0) as run:
            self.assertEqual(stack.main(self.arguments()), 0)
        self.assertEqual(run.call_args.kwargs, {})
        self.assertIsNone(run.call_args.args[0].in04_managed_inputs)

    def test_managed_flag_without_injected_actual_profile_refuses_before_run(self):
        with patch.object(stack, "run", side_effect=AssertionError("native started")):
            with patch("builtins.print"):
                self.assertEqual(stack.main(self.arguments() + ["--in04-managed-inputs", "/private/profile.json"]), 2)

    def test_injected_profile_requires_explicit_managed_input_path(self):
        with patch.object(stack, "run", side_effect=AssertionError("native started")):
            with patch("builtins.print"):
                self.assertEqual(stack.main(self.arguments(), managed=object()), 2)

    def test_injected_profile_and_explicit_path_forward_same_object(self):
        profile = object()
        with patch.object(stack, "run", return_value=0) as run:
            self.assertEqual(stack.main(self.arguments() + ["--in04-managed-inputs", "/private/profile.json"],
                                        managed=profile), 0)
        self.assertIs(run.call_args.kwargs["managed"], profile)
        self.assertEqual(run.call_args.args[0].in04_managed_inputs, Path("/private/profile.json"))

    def test_direct_api_missing_profile_refuses_before_input_or_native_work(self):
        with patch.object(stack, "load_helpers", side_effect=AssertionError("services imported")):
            with self.assertRaisesRegex(stack.InputRefusal, "^managed-profile-required$"):
                stack.run(SimpleNamespace(in04_managed_inputs=Path("/private/profile.json")))

    def test_incomplete_profile_refuses_before_loading_native_helpers(self):
        with tempfile.TemporaryDirectory(prefix="ma-cli-") as directory:
            path = Path(directory).resolve() / "profile.json"
            path.write_text("{}")
            path.chmod(0o600)
            with patch.object(stack, "load_helpers", side_effect=AssertionError("services imported")):
                with self.assertRaisesRegex(stack.InputRefusal, "^managed-profile-incomplete$"):
                    stack.run(SimpleNamespace(in04_managed_inputs=path), managed=object())


class ImmutableSources(unittest.TestCase):
    def test_migrations_use_binary_revision_even_after_checkout_changes(self):
        with tempfile.TemporaryDirectory(prefix="mcr-in03-source-") as directory:
            root = Path(directory).resolve()
            def git(*args):
                return subprocess.run(["/usr/bin/git", "-C", str(root), *args], check=True,
                                      stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout.decode().strip()
            git("init", "-q")
            path = root / "backend/services/concert-service/migrations/0051_social.sql"
            path.parent.mkdir(parents=True)
            original = b"SELECT 51;\n"
            path.write_bytes(original)
            config = root / "backend/services/concert-service/src/config/mod.rs"
            config.parent.mkdir(parents=True)
            config.write_text('const PACKAGED_OVERRIDE: &str = "CONCERT_MIGRATION_DIRECTORY";\n')
            git("add", "--", str(path), str(config))
            git("-c", "user.name=IN03 pure test", "-c", "user.email=in03@localhost", "commit", "-qm", "fixture")
            revision = git("rev-parse", "HEAD")
            path.write_bytes(b"SELECT uncommitted_different_sql;\n")
            pin = stack.BinaryPin(root / "unused", "0" * 64, revision)
            result = stack.migration_inputs(root, {"concert-service": pin})
            self.assertEqual(result["concert-service"], {51: hashlib.sha384(original).hexdigest()})
            owned = root / "owned"
            owned.mkdir(mode=0o700)
            cluster = SimpleNamespace(root=owned, assert_owned=Mock())
            package = stack.package_concert_migrations(root, pin, cluster, result["concert-service"])
            cluster.assert_owned.assert_called_once_with()
            packaged = package / path.name
            self.assertEqual(packaged.read_bytes(), original)
            self.assertEqual(packaged.stat().st_mode & 0o777, 0o600)
            # Checkout additions and edits never enter the runtime SQL package.
            path.with_name("0060_future.sql").write_text("SELECT 60;\n")
            stack.verify_migration_package(package, result["concert-service"])
            packaged.write_bytes(b"SELECT altered_package;\n")
            with self.assertRaisesRegex(stack.InputRefusal, "^migration-package-source-mismatch$"):
                stack.verify_migration_package(package, result["concert-service"])
            packaged.write_bytes(original)
            extra = package / "0060_extra.sql"
            extra.write_bytes(b"SELECT 60;\n")
            extra.chmod(0o600)
            with self.assertRaisesRegex(stack.InputRefusal, "^migration-package-source-mismatch$"):
                stack.verify_migration_package(package, result["concert-service"])
            extra.unlink()
            packaged.unlink()
            packaged.symlink_to(path)
            with self.assertRaisesRegex(stack.InputRefusal, "^explicit-file-path-required$"):
                stack.verify_migration_package(package, result["concert-service"])

    def test_packaging_rejects_old_main_source_before_creating_runtime_directory(self):
        with tempfile.TemporaryDirectory(prefix="mcr-in03-old-package-") as directory:
            root = Path(directory).resolve()
            cluster = SimpleNamespace(root=root, assert_owned=Mock())
            pin = stack.BinaryPin(root / "unused", "0" * 64, "a" * 40)
            with patch.object(stack, "source_bytes", return_value=b"compile-checkout-only migration path"):
                with self.assertRaisesRegex(stack.InputRefusal, "^migration-package-override-source-required$"):
                    stack.package_concert_migrations(root, pin, cluster, {51: "a" * 96})
            self.assertFalse((root / "concert-migration-package").exists())

    def test_old_world_composition_is_refused_even_when_relabelled_advanced(self):
        pins = {name: stack.BinaryPin(Path("/unused"), "0" * 64, "a" * 40) for name in stack.SERVICES}
        with patch.object(stack, "source_bytes", return_value=b"World-only startup"):
            with self.assertRaisesRegex(stack.InputRefusal, "^advanced-composition-source-required$"):
                stack.advanced_sources(Path("/unused"), pins)


if __name__ == "__main__":
    unittest.main()
