"""Offline ownership and failure regressions, never real Auth/world acceptance."""
import argparse
import contextlib
import io
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
from uuid import uuid4

import in01_provision as p


class Fixture(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="mcr-in01-tests-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()

    def run_dir(self):
        run = p.PrivateRun(self.root / "run")
        self.addCleanup(run.close)
        return run


class PrivateTests(Fixture):
    def test_exclusive_private_checkpoint_and_existing_refusal(self):
        run = self.run_dir()
        run.write("private.json", {"password": "only-in-private-file"})
        run.write("private.json", {"password": "updated-private-value"})
        self.assertEqual((run.path / "private.json").stat().st_mode & 0o777, 0o600)
        self.assertEqual(run.path.stat().st_mode & 0o777, 0o700)
        with self.assertRaises(FileExistsError):
            p.PrivateRun(run.path)
        self.assertEqual(run.read("private.json")["password"], "updated-private-value")

    def test_refuses_symlink_file_without_overwriting_target(self):
        run = self.run_dir()
        target = self.root / "foreign"
        target.write_text("preserve")
        (run.path / "private.json").symlink_to(target)
        with self.assertRaises(p.Failure):
            run.write("private.json", {"password": "secret"})
        self.assertEqual(target.read_text(), "preserve")
        with self.assertRaises(OSError):
            p.read_regular(run.path / "private.json", private=True)

    def test_parent_symlink_and_repository_output_refused(self):
        (self.root / "link").symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(p.Failure):
            p.PrivateRun(self.root / "link" / "secret")
        with self.assertRaises(p.Failure):
            p.PrivateRun(p.ROOT / "private-in01-secret")
        self.assertFalse((p.ROOT / "private-in01-secret").exists())

    def test_replaced_output_directory_cannot_receive_checkpoint(self):
        run = self.run_dir()
        run.path.rename(self.root / "old")
        run.path.mkdir(mode=0o700)
        with self.assertRaises(p.Failure):
            run.write("private.json", {"password": "secret"})
        self.assertFalse((run.path / "private.json").exists())

    def test_insecure_or_foreign_marker_and_file_refused(self):
        run = self.run_dir()
        run.write("private.json", {})
        (run.path / "private.json").chmod(0o644)
        with self.assertRaises(p.Failure):
            p.read_regular(run.path / "private.json", private=True)
        run.write("owner.json", {"kind": "foreign"})
        with self.assertRaises(p.Failure):
            p.PrivateRun(run.path, existing=True)

    def test_descriptor_control_denial_precedes_http_or_mail(self):
        path = self.root / "ready.json"
        path.write_text('{}'); path.chmod(0o600)
        with patch.object(p.dev, "control_request", return_value={"state": "REJECTED"}), self.assertRaises(p.Failure):
            p.owned_descriptor(path)

    def test_changed_or_missing_binary_receipt_refuses_launch(self):
        receipt = self.root / "build.json"
        receipt.write_text(json.dumps({"source_revision": "a" * 40, "exit_code": 0, "binary_sha256": {}}))
        with self.assertRaises(p.Failure):
            p.validate_binaries(self.root, receipt)
        receipt.write_text(json.dumps({"source_revision": "a" * 40, "exit_code": 1}))
        with self.assertRaises(p.Failure):
            p.validate_binaries(self.root, receipt)

    def test_source_drift_is_explicitly_historical_and_cannot_be_default(self):
        hashes={}
        for service in p.dev.runtime.START_ORDER:
            path=self.root/service;path.write_bytes(b"offline-artifact");path.chmod(0o700)
            hashes[service]=p.digest(path.read_bytes())
        receipt=self.root/"build.json"
        receipt.write_text(json.dumps({"source_revision":"a"*40,"exit_code":0,"binary_sha256":hashes,
                                       "source_files_sha256":{"backend/services/auth-service/src/main.rs":"0"*64}}))
        with self.assertRaisesRegex(p.Failure,"BINARY_SOURCE_DRIFT_REQUIRES_HISTORICAL_AUTH_ONLY"):
            p.validate_binaries(self.root,receipt)
        binding=p.validate_binaries(self.root,receipt,historical_auth_only=True)
        self.assertEqual(binding["qualification_scope"],"HISTORICAL_AUTH_ONLY_NOT_CURRENT_WORLD")
        self.assertEqual(len(binding["current_source_drift"]),1)
        descriptor = {"in01_qualification": {"scope": binding["qualification_scope"],
                                               "bin_dir": str(self.root), "receipt": p.pinned(receipt)}}
        self.assertEqual(p.attached_qualification(descriptor), binding)
        descriptor["in01_qualification"]["receipt"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(p.Failure, "OWNER_QUALIFICATION_CHANGED"):
            p.attached_qualification(descriptor)
        self.assertEqual(p.attached_qualification({})["qualification_scope"],
                         "UNVERIFIED_OWNER_ARTIFACTS_NO_GATE_CONFIG")

    def test_mail_matches_exact_owned_generated_email(self):
        mail = self.root / "mail"; mail.mkdir(mode=0o700)
        for name, email, code in [("other", "other@example.test", "111111"), ("own", "own@example.test", "222222")]:
            path = mail / (name + ".json")
            path.write_text(json.dumps({"kind": "VERIFY_EMAIL", "payload": {"email": email, "verification_code": code}}));path.chmod(0o600)
        self.assertEqual(p.wait_mail(mail, "own@example.test"), "222222")
        with self.assertRaisesRegex(p.Failure, "ACTUAL_VERIFICATION_MAIL_TIMEOUT"):
            p.wait_mail(mail, "absent@example.test", timeout=0)

    def test_fake_fixture_resources_never_become_reviewed_art(self):
        path = self.root / "fixture.json"
        path.write_text(json.dumps({"kind": "map_manifest", "provenance": {"source_kind": "ORIGINAL_FIXTURE", "review_state": "REVIEWED", "reviewed_by": "fixture", "reviewed_at": "2030-01-01T00:00:00Z"}}))
        with self.assertRaises(p.Failure):
            p.resource_pins(path, path, path)
        with self.assertRaises(p.Failure):
            p.resource_pins(None, None, None)

    def test_gate_config_has_closed_fe_shape_without_tokens(self):
        receipt = self.root / "receipt.json";receipt.write_text('{}')
        state = {"accounts": [{"email": f"a{i}@example.test", "password": "private-password", "userId": str(uuid4()), "accessToken": "private-token", "cookie": "private-cookie"} for i in range(2)]}
        config = p.gate_config(state, "http://127.0.0.1:12345", str(uuid4()), "a" * 40, receipt, receipt)
        self.assertEqual(set(config), {"version", "isolated", "origin", "eventId", "releaseRevision", "accounts", "serviceReceipt", "artReview"})
        self.assertNotIn("private-token", json.dumps(config));self.assertNotIn("private-cookie", json.dumps(config))
        state["accounts"][1]["userId"] = state["accounts"][0]["userId"]
        with self.assertRaises(p.Failure):
            p.gate_config(state, "http://localhost:12345", str(uuid4()), "a" * 40, receipt, receipt)

    def test_invalid_gate_origins_match_fe_rejections(self):
        for origin in ["http://example.test", "https://host/path", "https://user@host", "https://host:bad", "https://HOST", "https://host:443", "http://localhost:80", "http://[::1", "https://host?q=secret"]:
            with self.subTest(origin=origin), self.assertRaises(p.Failure):
                p.validate_origin(origin)
        for origin in ["https://host", "http://localhost", "http://127.0.0.1:12121", "http://[::1]:5"]:
            self.assertEqual(p.validate_origin(origin), origin)


class FailureTests(Fixture):
    def args(self):
        return argparse.Namespace(bin_dir=self.root, binary_receipt=self.root / "receipt", ready_file=self.root / "ready", browser_origin=None,
                                  event_id=None, map_manifest=None, avatar_manifest=None, art_review=None,
                                  release_revision="a" * 40, hold_seconds=0)

    def test_mid_provision_failure_closes_owned_stack_and_sanitizes_error(self):
        run = self.run_dir(); stack = Mock(); stack.report = {"cleanup": "PASS"}
        stack.start.return_value = {"gateway_url": "http://127.0.0.1:12345"}
        with patch.object(p, "source_receipt", return_value={}), patch.object(p, "validate_binaries", return_value={"qualification_scope":"SOURCE_HASHES_MATCH_BUILD_RECEIPT","receipt":{}}), patch.object(p.dev, "Control"), patch.object(p, "provision_accounts", side_effect=RuntimeError("PRIVATE_PASSWORD_MUST_NOT_LEAK")), contextlib.redirect_stdout(io.StringIO()):
            result = p.execute(self.args(), run, stack)
        stack.close.assert_called_once()
        self.assertEqual(result["result"], "FAIL");self.assertEqual(result["cleanup"], "PASS")
        self.assertEqual(result["failure"], "RuntimeError")
        self.assertNotIn("PRIVATE_PASSWORD_MUST_NOT_LEAK", (run.path / "receipt.json").read_text())

    def test_cleanup_failure_cannot_leave_green_provision_result(self):
        run = self.run_dir(); stack = Mock();stack.close.side_effect = OSError("private OS diagnostic")
        with patch.object(p, "source_receipt", return_value={}), patch.object(p, "validate_binaries", side_effect=p.Failure("BINARY_RECEIPT_MISMATCH")):
            result = p.execute(self.args(), run, stack)
        stack.start.assert_not_called();self.assertEqual(result["result"], "FAIL");self.assertEqual(result["cleanup"], "FAIL")
        self.assertEqual(result["lifetime"], "OWNED_CLEANUP_UNCONFIRMED_DATA_MAY_REMAIN")

    def test_unqualified_attach_never_emits_config_despite_successful_reads(self):
        art = self.root / "supplied-art-receipt.json"; art.write_text('{}')
        for scope in ("HISTORICAL_AUTH_ONLY_NOT_CURRENT_WORLD", "UNVERIFIED_OWNER_ARTIFACTS_NO_GATE_CONFIG"):
            with self.subTest(scope=scope):
                run = p.PrivateRun(self.root / scope)
                self.addCleanup(run.close)
                args = self.args(); args.event_id = str(uuid4()); args.art_review = art
                args.browser_origin = "http://127.0.0.1:12345"
                with patch.object(p, "source_receipt", return_value={}), patch.object(p, "owned_descriptor", return_value=({}, "offline")), patch.object(p, "attached_qualification", return_value={"qualification_scope":scope}), patch.object(p, "provision_accounts", return_value={"accounts":[]}), patch.object(p, "check_world", return_value=True), patch.object(p, "gate_config") as config, contextlib.redirect_stdout(io.StringIO()):
                    result = p.execute(args, run)
                self.assertEqual(result["result"], "PASS")
                self.assertEqual(result["binary_bindings"]["qualification_scope"], scope)
                config.assert_not_called()
                self.assertFalse((run.path / "config.json").exists())

    def test_owned_lifecycle_failure_is_not_masked_by_successful_file_cleanup(self):
        run = self.run_dir(); stack = Mock(); stack.report = {"result":"FAIL", "cleanup":"PASS"}
        stack.start.return_value = {}
        with patch.object(p, "source_receipt", return_value={}), patch.object(p, "validate_binaries", return_value={"qualification_scope":"SOURCE_HASHES_MATCH_BUILD_RECEIPT", "receipt":{}}), patch.object(p.dev, "Control"), patch.object(p, "provision_accounts", return_value={"accounts":[]}), patch.object(p, "initialize_event", return_value=str(uuid4())), patch.object(p, "check_world", return_value=False), contextlib.redirect_stdout(io.StringIO()):
            result = p.execute(self.args(), run, stack)
        stack.close.assert_called_once()
        self.assertEqual(result["result"], "FAIL")
        self.assertEqual(result["cleanup"], "PASS")
        self.assertEqual(result["lifetime"], "ENDED_ACCOUNTS_AND_EVENT_REMOVED_WITH_OWNED_CLUSTER")

    def test_returned_cleanup_failure_never_claims_removal(self):
        run = self.run_dir(); stack = Mock(); stack.report = {"result":"FAIL", "cleanup":"FAIL"}
        with patch.object(p, "source_receipt", return_value={}), patch.object(p, "validate_binaries", side_effect=p.Failure("BINARY_RECEIPT_MISMATCH")):
            result = p.execute(self.args(), run, stack)
        self.assertEqual(result["result"], "FAIL")
        self.assertEqual(result["lifetime"], "OWNED_CLEANUP_UNCONFIRMED_DATA_MAY_REMAIN")

    def test_partial_signup_keeps_only_private_recoverable_credentials(self):
        run = self.run_dir(); report={"statuses": {}}
        browser = Mock();browser.request.side_effect = auth_error = p.auth.CheckFailure("account-0-signup:status")
        descriptor = {"gateway_url": "http://127.0.0.1:12345", "root": str(self.root)}
        with patch.object(p.auth, "Browser", return_value=browser), self.assertRaises(p.auth.CheckFailure):
            p.provision_accounts(run, descriptor, report)
        state = run.read("private.json")
        self.assertEqual(state["accounts"][0]["stage"], "SIGNUP_PENDING")
        self.assertEqual(state["accounts"][1]["stage"], "GENERATED")
        self.assertNotIn(state["accounts"][0]["password"], json.dumps(report))
        self.assertNotIn(state["accounts"][0]["email"], str(auth_error))

    def test_initializer_requires_real_new_cluster_capability_before_sql(self):
        stack = Mock();stack.cluster = object()
        with self.assertRaises(p.Failure):
            p.initialize_event(stack, "a" * 32, {})

    def test_event_database_uses_unchanged_owned_url_namespace(self):
        cluster = object.__new__(p.dev.migration.Cluster)
        cluster.password = "offline-only-password"
        cluster.socket = self.root
        with patch.object(cluster, "assert_owned") as ownership:
            value = cluster.url(p.DATABASE)
        ownership.assert_called_once()
        self.assertIn("/" + p.DATABASE + "?", value)
        self.assertTrue(p.DATABASE.startswith("mcr_concert_"))

    def test_shutdown_failure_still_calls_stack_cleanup(self):
        run=self.run_dir();stack=Mock();stack.report={"cleanup":"PASS"}
        stack.start.return_value = {}
        control=Mock();control.server_close.side_effect=OSError("private diagnostic")
        with patch.object(p,"source_receipt",return_value={}), patch.object(p,"validate_binaries",return_value={"qualification_scope":"SOURCE_HASHES_MATCH_BUILD_RECEIPT","receipt":{}}), patch.object(p.dev,"Control",return_value=control), patch.object(p,"provision_accounts",side_effect=p.Failure("SIGNUP_UNAVAILABLE")):
            result=p.execute(self.args(),run,stack)
        stack.close.assert_called_once()
        self.assertEqual(result["cleanup"],"FAIL")

    def test_receipt_scrubs_inherited_private_diagnostics(self):
        run=self.run_dir()
        run.write("private.json",{"accounts":[{"email":"private@example.test","password":"secret-password","accessToken":"private-access","cookie":"private-cookie"}]})
        report={"log":["private@example.test secret-password private-access private-cookie"],"result":"FAIL"}
        sanitized=p.save_receipt(run,report)
        self.assertEqual(sanitized["log"],["[private] [private] [private] [private]"])
        self.assertNotIn("secret-password",(run.path/"receipt.json").read_text())

    def test_invalid_event_preflight_allocates_no_stack_or_accounts(self):
        run=self.run_dir();stack=Mock();stack.report={"cleanup":"PASS"}
        args=self.args();args.event_id="invalid"
        with patch.object(p,"source_receipt",return_value={}):
            result=p.execute(args,run,stack)
        stack.start.assert_not_called()
        self.assertEqual(result["failure"],"INVALID_EVENT_ID")
        self.assertFalse((run.path/"private.json").exists())


class ConcertTests(Fixture):
    def artifact(self):
        paths = set(p.CONCERT_DRIVER_PATHS) | {"backend/services/concert-service/src/main.rs", "backend/Cargo.lock"}
        for relative in paths:
            path = self.root / relative; path.parent.mkdir(parents=True,exist_ok=True); path.write_bytes(b"offline-build-input")
        binary = self.root / "concert-service"; binary.write_bytes(b"offline-executable"); binary.chmod(0o700)
        receipt = self.root / "concert-receipt.json"
        value = {"exit_code":0,"source_revision":"a"*40,"binary_sha256":{"concert-service":p.digest(binary.read_bytes())},
                 "source_files_sha256":{relative:p.digest((self.root/relative).read_bytes()) for relative in paths}}
        receipt.write_text(json.dumps(value))
        return binary,receipt,value,paths

    def test_driver_only_differences_are_named_and_hashed(self):
        binary,receipt,value,paths = self.artifact()
        for relative in p.CONCERT_DRIVER_PATHS:
            (self.root/relative).write_bytes(b"new-driver-only")
        with patch.object(p,"ROOT",self.root),patch.object(p,"backend_source_paths",return_value=paths):
            binding=p.validate_concert_binary(binary,receipt)
        self.assertEqual(binding["source_revision"],"a"*40)
        self.assertEqual(binding["source_manifest_count"],len(paths))
        self.assertEqual(binding["qualification_scope"],"RUNTIME_SOURCES_MATCH_BUILD_RECEIPT_DRIVER_DIFFERENCES_RECORDED")
        self.assertEqual({d["path"] for d in binding["driver_source_differences"]},p.CONCERT_DRIVER_PATHS)
        self.assertTrue(all(d["build_sha256"]!=d["execution_sha256"] for d in binding["driver_source_differences"]))

    def test_runtime_or_lock_drift_is_not_a_driver_exception(self):
        binary,receipt,value,paths=self.artifact()
        for relative in ("backend/services/concert-service/src/main.rs","backend/Cargo.lock"):
            original=(self.root/relative).read_bytes();(self.root/relative).write_bytes(b"unbuilt-runtime-change")
            with self.subTest(path=relative),patch.object(p,"ROOT",self.root),patch.object(p,"backend_source_paths",return_value=paths),self.assertRaisesRegex(p.Failure,"CONCERT_RUNTIME_SOURCE_DRIFT"):
                p.validate_concert_binary(binary,receipt)
            (self.root/relative).write_bytes(original)

    def test_changed_binary_or_missing_manifest_is_refused(self):
        binary,receipt,value,paths=self.artifact()
        with patch.object(p,"ROOT",self.root),patch.object(p,"backend_source_paths",return_value=paths):
            original=binary.read_bytes();binary.write_bytes(b"changed-artifact")
            with self.assertRaisesRegex(p.Failure,"CONCERT_BINARY_RECEIPT_MISMATCH"):
                p.validate_concert_binary(binary,receipt)
            binary.write_bytes(original)
            value["source_files_sha256"].pop("backend/Cargo.lock");receipt.write_text(json.dumps(value))
            with self.assertRaisesRegex(p.Failure,"CONCERT_COMPLETE_BACKEND_SOURCE_MANIFEST_REQUIRED"):
                p.validate_concert_binary(binary,receipt)

    def test_failed_build_and_unsafe_source_pin_are_refused(self):
        binary,receipt,value,paths=self.artifact()
        value["exit_code"]=1;receipt.write_text(json.dumps(value))
        with self.assertRaisesRegex(p.Failure,"CONCERT_BUILD_RECEIPT_REQUIRED"):
            p.validate_concert_binary(binary,receipt)
        value["exit_code"]=0
        value["source_files_sha256"]["backend/../foreign"]="0"*64
        receipt.write_text(json.dumps(value))
        with patch.object(p,"ROOT",self.root),patch.object(p,"backend_source_paths",return_value=set(value["source_files_sha256"])),self.assertRaisesRegex(p.Failure,"CONCERT_SOURCE_PIN_INVALID"):
            p.validate_concert_binary(binary,receipt)

    def stack(self):
        cluster=object.__new__(p.dev.migration.Cluster)
        cluster.root=self.root;cluster.env={}
        restarted={"value":False}
        cluster.assert_owned=Mock()
        cluster.assert_postmaster=Mock(side_effect=lambda:202 if restarted["value"] else 101)
        cluster.url=Mock(return_value="postgresql://offline-owner@localhost/mcr_concert_in")
        def sql(database,query):
            if query=="select pg_postmaster_start_time();":return "after" if restarted["value"] else "before"
            if query.startswith("select phase||"):return "OPEN:1"
            return ""
        cluster.sql=Mock(side_effect=sql)
        cluster.stop=Mock();cluster.start=Mock(side_effect=lambda:restarted.update(value=True))
        stack=Mock();stack.cluster=cluster;stack.report={"cleanup":"PASS","result":"PASS"};stack.start.return_value={}
        return stack

    def test_full_cli_mode_repeats_and_restarts_without_five_file_fallback(self):
        stack=self.stack();report={}
        with patch.object(p.dev.migration,"execute",return_value=SimpleNamespace(returncode=0)) as cli,patch.object(p.dev.migration,"check_ledger",return_value=12) as ledger:
            p.initialize_event(stack,"a"*32,report,concert_binary=self.root/"concert-service")
        self.assertEqual([call.args[0][-1] for call in cli.call_args_list],["--migrate","--verify-migrations","--migrate","--verify-migrations"])
        self.assertEqual(ledger.call_count,3)
        self.assertTrue(all(len(call.args[2])==12 for call in ledger.call_args_list))
        stack.cluster.stop.assert_called_once();stack.cluster.start.assert_called_once();stack.health.assert_called_once()
        self.assertEqual(report["concert_migrations"]["mode"],"CURRENT_BINARY_FULL_SQLX")
        self.assertEqual(report["concert_migrations"]["status"],"PASS")
        self.assertEqual(len(report["fixture"]["migration_hashes"]),12)
        self.assertFalse(any("CREATE TABLE" in call.args[1].upper() for call in stack.cluster.sql.call_args_list))

    def test_bounded_mode_never_claims_a_sqlx_ledger_or_calls_cli(self):
        stack=self.stack();report={}
        with patch.object(p.dev.migration,"execute") as cli,patch.object(p.dev.migration,"check_ledger") as ledger:
            p.initialize_event(stack,"a"*32,report)
        cli.assert_not_called();ledger.assert_not_called();stack.cluster.stop.assert_not_called()
        self.assertEqual(len(report["fixture"]["migration_hashes"]),5)
        self.assertEqual(report["concert_migrations"]["mode"],"BOUNDED_DIRECT_SCHEMA_NO_SQLX_LEDGER")

    def test_concert_cli_failure_cleans_stack_and_cannot_fall_back(self):
        run=self.run_dir();stack=self.stack();args=FailureTests.args(self)
        args.concert_binary=self.root/"concert-service";args.concert_build_receipt=self.root/"build.json"
        error=p.dev.SmokeError("concert-migrate","private diagnostic",1)
        with patch.object(p,"source_receipt",return_value={}),patch.object(p,"validate_concert_binary",return_value={}),patch.object(p,"validate_binaries",return_value={"qualification_scope":"SOURCE_HASHES_MATCH_BUILD_RECEIPT","receipt":{}}),patch.object(p.dev,"Control"),patch.object(p,"provision_accounts",return_value={"accounts":[]}),patch.object(p.dev.migration,"execute",side_effect=error):
            result=p.execute(args,run,stack)
        stack.close.assert_called_once()
        self.assertEqual(result["failure"],"CONCERT_CLI_MIGRATION_FAILED")
        self.assertEqual(result["concert_migrations"]["status"],"FAIL")
        self.assertEqual(result["concert_migrations"]["commands"][0]["exit_code"],1)
        self.assertNotIn("fixture",result)
        self.assertFalse(any("insert into concert_events" in call.args[1] for call in stack.cluster.sql.call_args_list))

    def test_unqualified_concert_receipt_refuses_before_startup_or_accounts(self):
        run=self.run_dir();stack=self.stack();args=FailureTests.args(self)
        args.concert_binary=self.root/"concert-service";args.concert_build_receipt=self.root/"build.json"
        with patch.object(p,"source_receipt",return_value={}),patch.object(p,"validate_concert_binary",side_effect=p.Failure("CONCERT_RUNTIME_SOURCE_DRIFT")),patch.object(p,"provision_accounts") as accounts:
            result=p.execute(args,run,stack)
        stack.start.assert_not_called();accounts.assert_not_called();stack.close.assert_called_once()
        self.assertEqual(result["result"],"FAIL")
        self.assertEqual(result["failure"],"CONCERT_RUNTIME_SOURCE_DRIFT")

    def test_full_ledger_mismatch_cleans_stack_without_inserting_event(self):
        run=self.run_dir();stack=self.stack();args=FailureTests.args(self)
        args.concert_binary=self.root/"concert-service";args.concert_build_receipt=self.root/"build.json"
        with patch.object(p,"source_receipt",return_value={}),patch.object(p,"validate_concert_binary",return_value={}),patch.object(p,"validate_binaries",return_value={"qualification_scope":"SOURCE_HASHES_MATCH_BUILD_RECEIPT","receipt":{}}),patch.object(p.dev,"Control"),patch.object(p,"provision_accounts",return_value={"accounts":[]}),patch.object(p.dev.migration,"execute",return_value=SimpleNamespace(returncode=0)),patch.object(p.dev.migration,"check_ledger",side_effect=p.dev.SmokeError("ledger","offline checksum mismatch")):
            result=p.execute(args,run,stack)
        stack.close.assert_called_once()
        self.assertEqual(result["failure"],"CONCERT_FULL_LEDGER_MISMATCH")
        self.assertEqual(result["concert_migrations"]["status"],"FAIL")
        self.assertNotIn("fixture",result)
        self.assertFalse(any("insert into concert_events" in call.args[1] for call in stack.cluster.sql.call_args_list))


class WorldReadTests(Fixture):
    def server(self, *, status=200, cache_control="no-store"):
        event, room = str(uuid4()), str(uuid4())
        accounts = [{"userId":str(uuid4()), "accessToken":"offline-token-" + str(i)} for i in range(2)]
        seen = []
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                body_size = int(self.headers.get("Content-Length", "0"))
                seen.append((body_size, self.headers.get("Content-Type"), self.headers.get("X-MCR-Intent")))
                actual = 413 if body_size or self.headers.get("Transfer-Encoding") else status
                account = next(a for a in accounts if self.headers.get("Authorization") == "Bearer " + a["accessToken"])
                value = ({"event":{"event_id":event}, "participant":{"user_id":account["userId"]},
                          "capabilities":{"world":{"available":True}}} if self.path.endswith("/bootstrap") else
                         {"event_id":event,"default_room_id":room,"items":[{"room_id":room,"availability":"AVAILABLE"}]})
                raw = json.dumps(value).encode()
                self.send_response(actual)
                self.send_header("cache-control", cache_control)
                self.send_header("x-correlation-id", self.headers["X-Correlation-Id"])
                self.send_header("referrer-policy", "no-referrer")
                self.send_header("Content-Length", str(len(raw)))
                self.end_headers(); self.wfile.write(raw)
            def log_message(self, *_):
                pass
        server = ThreadingHTTPServer(("127.0.0.1",0),Handler)
        worker = threading.Thread(target=server.serve_forever, kwargs={"poll_interval":.01}, daemon=True)
        worker.start()
        def close():
            server.shutdown(); server.server_close(); worker.join(timeout=2)
        self.addCleanup(close)
        return {"gateway_url":f"http://127.0.0.1:{server.server_port}"}, {"accounts":accounts}, event, seen

    def test_real_http_reads_are_bodyless_and_scope_both_accounts(self):
        descriptor, state, event, seen = self.server()
        report = {"statuses":{}}
        self.assertTrue(p.check_world(descriptor, state, event, report))
        self.assertEqual(len(report["statuses"]),4)
        self.assertEqual(seen,[(0,None,"browser")]*4)
        self.assertEqual(report["world_prerequisites"]["status"],"PASS")

    def test_unexpected_rejection_is_not_a_missing_prerequisite(self):
        descriptor, state, event, _ = self.server(status=413)
        report = {"statuses":{}}
        with self.assertRaisesRegex(p.Failure,"REAL_WORLD_READ_REJECTED"):
            p.check_world(descriptor,state,event,report)
        self.assertEqual(report["statuses"]["account-0-bootstrap"],413)

    def test_missing_owner_route_stays_explicitly_not_run(self):
        descriptor, state, event, _ = self.server(status=503)
        report = {"statuses":{}}
        self.assertFalse(p.check_world(descriptor,state,event,report))
        self.assertEqual(report["world_prerequisites"]["observed_status"],503)
        self.assertEqual(report["world_prerequisites"]["status"],"NOT_RUN")

    def test_success_status_cannot_mask_response_policy_failure(self):
        descriptor, state, event, _ = self.server(cache_control="public")
        report = {"statuses":{}}
        with self.assertRaisesRegex(p.Failure,"REAL_WORLD_RESPONSE_POLICY_FAILED"):
            p.check_world(descriptor,state,event,report)


if __name__ == "__main__":
    unittest.main()
