"""Bounded offline regressions. No Auth, PG, native service or listener runs."""
import copy
import json
import re
import tempfile
import threading
import time
from pathlib import Path
from types import SimpleNamespace as NS
import unittest
from unittest.mock import AsyncMock, Mock, patch
from uuid import uuid4

import world_in01_live as live
import world_in01_prepare as prep
from test_world_capacity_probe import configuration
from test_world_live_failure_probe import FixtureNetwork
from world_capacity_support.report import Failure
from world_capacity_support.scenarios import Runner
from world_capacity_support.transport import Contracts


def arguments(profile="qa05"):
    now = time.time()
    return NS(qualification_profile=profile, allocation_id="FIXTURE-NOT-ACTUAL-ALLOCATION",
              provision_timeout_seconds=120, probe_timeout_seconds=300, measurement_seconds=20,
              input_hz=15, arrival_concurrency=4, restart_world=False,
              qa08_provisioning_budget="signup=400,login=200" if profile == "qa08" else None,
              whole_timeout_seconds=600, whole_byte_limit=1073741824, retained_carry_bytes=309342208,
              allocation_work_deadline_epoch=now+360, allocation_release_deadline_epoch=now+600,
              retain_public_projection=False,
              frontend_source_revision=None,
              browser_origin="http://localhost:5173", world=True, hold_seconds=0)


def handoff(profile="qa05"):
    args = arguments(profile)
    config = configuration(profile == "qa08")
    config["rooms"] = [{"room_id": r["room_id"], "capacity": cap}
                       for r, cap in zip(config["rooms"], prep.producer_profile(profile)["capacities"])]
    principals = [(str(uuid4()), Path("/not-read"), p["bearer"]) for p in config["principals"]]
    return args, config, principals


class ProfileTests(unittest.TestCase):
    def test_default_in01_and_original_cohorts(self):
        self.assertEqual(prep.producer_profile(), {"name": "in01", "principals": 2, "capacities": (2,)})
        self.assertEqual(prep.producer_profile("qa05")["capacities"], (2, 2))
        self.assertEqual(prep.producer_profile("qa08")["principals"], 200)
        for value in ("arbitrary", [], 0):
            with self.assertRaises(prep.auth.CheckFailure): prep.producer_profile(value)

    def test_provisioning_calls_real_principal_route_and_stops_on_duplicate(self):
        stack, browser = object(), object()
        first, second = (str(uuid4()), None, "token-a"), (str(uuid4()), None, "token-b")
        with patch.object(prep, "principal", side_effect=[first, second]) as create:
            self.assertEqual(prep.provision_principals(stack, browser, "qa05"), [first, second])
        self.assertEqual(create.call_count, 2)
        with patch.object(prep, "principal", side_effect=[first, first]):
            with self.assertRaisesRegex(prep.auth.CheckFailure, "distinct-canonical"):
                prep.provision_principals(stack, browser, "qa05")

    def test_normal_auth_refusal_is_not_retried_or_bypassed(self):
        with patch.object(prep, "principal", side_effect=prep.auth.CheckFailure("principal-1-signup:status")) as create:
            with self.assertRaises(prep.auth.CheckFailure): prep.provision_principals(None, None, "qa08")
        self.assertEqual(create.call_count, 1)

    def test_provisioning_deadline_stops_before_next_account(self):
        barrier, clock = threading.Barrier(4), [0]
        def finish(*args, **kwargs):
            barrier.wait(timeout=2)
            clock[0] = 2
            return str(uuid4()), None, "token"
        with patch.object(prep, "time", NS(monotonic=lambda: clock[0])), patch.object(prep, "principal", side_effect=finish) as create:
            with self.assertRaisesRegex(prep.auth.CheckFailure, "principal-provision-deadline"):
                prep.provision_principals(None, None, "qa08", 1)
        self.assertEqual(create.call_count, 4)

    def test_real200_scheduler_bounds_overlap_and_preserves_ordinal_order(self):
        barrier, lock = threading.Barrier(4), threading.Lock()
        active, peak, ordinals = 0, 0, []
        def finish(stack, browser, ordinal, **kwargs):
            nonlocal active, peak
            self.assertIsNotNone(kwargs.get("signup_gate"))
            with lock:
                active += 1
                peak = max(peak, active)
                ordinals.append(ordinal)
            try:
                barrier.wait(timeout=2)
                return str(ordinal), None, "RAM-TOKEN-" + str(ordinal)
            finally:
                with lock: active -= 1
        browser = NS(report={})
        with patch.object(prep, "principal", side_effect=finish):
            result = prep.provision_principals(None, browser, "qa08")
        self.assertEqual([p[0] for p in result], [str(n) for n in range(1, 201)])
        self.assertEqual(sorted(ordinals), list(range(1, 201)))
        self.assertEqual((peak, active), (4, 0))
        self.assertEqual(browser.report["principal_provisioning"],
                         {"concurrency": 4, "started": 200, "completed": 200})
        self.assertNotIn("RAM-TOKEN", json.dumps(browser.report))

    def test_concurrent_refusal_drains_current_batch_and_starts_no_next_batch(self):
        barrier, lock, ordinals = threading.Barrier(4), threading.Lock(), []
        active = 0
        def refuse(stack, browser, ordinal, *, cancel_event, **kwargs):
            nonlocal active
            with lock:
                active += 1
                ordinals.append(ordinal)
            try:
                barrier.wait(timeout=2)
                if ordinal == 1:
                    raise prep.auth.CheckFailure("principal-1-signup:status")
                self.assertTrue(cancel_event.wait(timeout=2))
                raise prep.auth.CheckFailure("principal-provision-cancelled")
            finally:
                with lock: active -= 1
        with patch.object(prep, "principal", side_effect=refuse):
            with self.assertRaisesRegex(prep.auth.CheckFailure, "principal-1-signup:status"):
                prep.provision_principals(None, None, "qa08")
        self.assertEqual(sorted(ordinals), [1, 2, 3, 4])
        self.assertEqual(active, 0)

    def test_cancelled_principal_makes_no_http_request(self):
        stop, browser = threading.Event(), Mock()
        stop.set()
        with self.assertRaisesRegex(prep.auth.CheckFailure, "principal-provision-cancelled"):
            prep.principal(None, browser, 1, cancel_event=stop)
        browser.request.assert_not_called()

    def test_failure_code_never_retains_arbitrary_exception_text(self):
        for label in ("principal-provision-deadline", "principal-200-login:status"):
            self.assertEqual(live.qualification_failure_code(prep.auth.CheckFailure(label)), label)
        for error in (ValueError("PRIVATE-TOKEN"), prep.auth.CheckFailure("principal-1-login:PRIVATE-TOKEN")):
            self.assertEqual(live.qualification_failure_code(error), "UNCLASSIFIED")

    def test_real_principal_sequence_does_not_persist_token(self):
        user, token = str(uuid4()), "PRIVATE-TEST-TOKEN"
        browser = Mock()
        captured = {}
        def request(label, path, payload):
            captured.update(payload)
            if path.endswith("signup") or path.endswith("verify"):
                return {}, {"userId": user}
            return {"set-cookie": "refresh=fake; HttpOnly; SameSite=Lax; Path=/"}, {"userId": user, "accessToken": token, "refreshToken": ""}
        browser.request.side_effect = request
        mail = Mock()
        mail.is_symlink.return_value = False
        mail.stat.return_value = NS(st_mode=0o600)
        mail.read_bytes.side_effect = lambda: json.dumps({"kind": "VERIFY_EMAIL", "payload": {"email": captured["email"], "verification_code": "fixture-code"}}).encode()
        stack = NS(cluster=NS(root=Path("/fixture-only"), assert_owned=Mock()),
                   mail=NS(directory=NS(glob=lambda _: [mail])))
        with patch.object(prep, "private_json") as persist:
            item = prep.principal(stack, browser, 1)
        self.assertEqual(item[2], token)
        self.assertNotIn(token, json.dumps(persist.call_args.args[1]))
        self.assertEqual(persist.call_args.args[1]["access_token"], "MEMORY_ONLY")
        self.assertEqual([c.args[1] for c in browser.request.call_args_list], ["/api/auth/email/signup", "/api/auth/email/verify", "/api/auth/login"])

    def test_concurrent_real_principal_flows_keep_email_code_password_token_bindings(self):
        barrier, lock, accounts, messages, persisted = threading.Barrier(4), threading.Lock(), {}, [], []
        signup_gate, active_signup, peak_signup = threading.Lock(), 0, 0
        def request(label, path, payload):
            nonlocal active_signup, peak_signup
            email = payload["email"]
            if path.endswith("signup"):
                with lock:
                    active_signup += 1
                    peak_signup = max(peak_signup, active_signup)
                time.sleep(.005)
                ordinal = int(label.split("-")[1])
                user, code, token = str(uuid4()), str(100000 + ordinal), "PRIVATE-TOKEN-" + str(ordinal)
                mail = Mock()
                mail.is_symlink.return_value = False
                mail.stat.return_value = NS(st_mode=0o600)
                mail.read_bytes.return_value = json.dumps({"kind": "VERIFY_EMAIL", "payload":
                    {"email": email, "verification_code": code}}).encode()
                with lock:
                    accounts[email] = (user, code, payload["password"], token)
                    messages.append(mail)
                    active_signup -= 1
                return {}, {"userId": user}
            user, code, password, token = accounts[email]
            if path.endswith("verify"):
                self.assertEqual(payload["code"], code)
                return {}, {"userId": user}
            self.assertEqual(payload["password"], password)
            return {"set-cookie": "refresh=private; HttpOnly; SameSite=Lax; Path=/"}, {
                "userId": user, "accessToken": token, "refreshToken": ""}
        def inbox(_):
            with lock: return list(messages)
        def persist(path, value):
            with lock: persisted.append((path, value))
        browser = NS(request=request)
        stack = NS(cluster=NS(root=Path("/fixture-only"), assert_owned=Mock()),
                   mail=NS(directory=NS(glob=inbox)))
        def provision(ordinal):
            barrier.wait(timeout=2)
            return prep.principal(stack, browser, ordinal, signup_gate=signup_gate)
        with patch.object(prep, "private_json", side_effect=persist), prep.ThreadPoolExecutor(max_workers=4) as pool:
            futures = [pool.submit(provision, ordinal) for ordinal in range(1, 5)]
            results = [future.result(timeout=3) for future in futures]
        self.assertEqual((peak_signup, active_signup), (1, 0))
        self.assertEqual(len({item[0] for item in results}), 4)
        self.assertEqual(len({path for path, _ in persisted}), 4)
        for email, (user, _, password, token) in accounts.items():
            self.assertIn((user, stack.cluster.root / f"in01-principal-{token.rsplit('-', 1)[1]}.json", token), results)
            saved = next(value for _, value in persisted if value["user_id"] == user)
            self.assertEqual((saved["email"], saved["password"], saved["access_token"]),
                             (email, password, "MEMORY_ONLY"))
        self.assertNotIn("PRIVATE-TOKEN", json.dumps([value for _, value in persisted]))

    def test_signup_gate_serializes_only_signup_and_rechecks_cancellation(self):
        gate, stop, entered = threading.Lock(), threading.Event(), threading.Event()
        browser = Mock()
        gate.acquire()
        def waiting():
            entered.set()
            return prep.principal(None, browser, 1, cancel_event=stop, signup_gate=gate)
        with prep.ThreadPoolExecutor(max_workers=1) as pool:
            future = pool.submit(waiting)
            self.assertTrue(entered.wait(timeout=2))
            stop.set()
            gate.release()
            with self.assertRaisesRegex(prep.auth.CheckFailure, "principal-provision-cancelled"):
                future.result(timeout=2)
        browser.request.assert_not_called()

    def test_operator_seed_only_changes_room_configuration(self):
        bootstrap = configuration()["bootstrap"]
        for profile, capacities in (("in01", [2]), ("qa05", [2, 2]), ("qa08", [50]*4)):
            with self.subTest(profile=profile):
                statements = []
                def sql(database, query):
                    statements.append(query)
                    if query.startswith("BEGIN; ALTER"): return "f"
                    if query.startswith("SELECT count"): return "0"
                    return "t"
                cluster = NS(root=Path("/fixture-only"), env={}, tools={"createdb": Path("/not-executed")},
                             assert_owned=Mock(), client_env=lambda _: {}, url=lambda _: "private-db-fixture", sql=sql)
                with patch.object(prep.dev.migration, "executable", side_effect=lambda p:p), \
                     patch.object(prep.dev.migration, "execute"), \
                     patch.object(prep.dev.migration, "migration_manifest", return_value={}), \
                     patch.object(prep.dev.migration, "check_ledger", return_value=37) as ledger, \
                     patch.object(prep.dev.runtime, "digest", return_value="0"*64), \
                     patch.object(Path, "read_bytes", return_value=json.dumps(bootstrap).encode()), \
                     patch.object(Path, "read_text", return_value="SCHEMA-FIXTURE"), \
                     patch.object(prep, "release_inputs", return_value={}), \
                     patch.object(prep, "private_json") as persist:
                    runtime_manifest = {49: "a"*96} if profile == "qa05" else None  # Fixture override, not a runtime ledger.
                    prep.seed_concert(NS(cluster=cluster), Path("/not-executed"), {}, False, profile, runtime_manifest=runtime_manifest)
                    self.assertEqual(ledger.call_args.args[2], runtime_manifest if runtime_manifest is not None else {})
                rooms = json.loads(persist.call_args.args[1]["CONCERT_WORLD_ROOMS_JSON"])
                self.assertEqual([r["capacity"] for r in rooms], capacities)
                self.assertEqual(len({r["room_id"] for r in rooms}), len(rooms))
                insert = next(q for q in statements if "INSERT INTO concert_events" in q)
                self.assertEqual(insert.count("INSERT INTO concert_world_rooms"), len(rooms))
                for table in ("users", "concert_participants", "concert_event_admissions", "concert_avatar_settings"):
                    self.assertNotIn("INSERT INTO " + table, insert)

    def test_in_memory_handoff_balances_original_capacity(self):
        for profile in ("qa05", "qa08"):
            args, config, principals = handoff(profile)
            result = live.qualification_config(args, config, {"event": config["event_id"], "rooms": config["rooms"]}, principals,
                                               config["bootstrap"], config["ws_url"])
            self.assertEqual(len(result["principals"]), len(principals))
            self.assertEqual(result["principals"][0]["bearer"], principals[0][2])
            if profile == "qa08":
                self.assertEqual([sum(p["room_index"] == r for p in result["principals"]) for r in range(4)], [50]*4)
                self.assertEqual(result["schedule"]["http_interval_seconds"], .12)
            else:
                self.assertIn("room_switch", result["faults"])

    def test_invalid_flags_fail_before_starting_resources(self):
        for key, value in (("world", False), ("hold_seconds", 2), ("allocation_id", None), ("input_hz", float("nan"))):
            args = arguments()
            setattr(args, key, value)
            with self.assertRaises((prep.auth.CheckFailure, Failure)): live.validate_qualification_arguments(args)
        args = arguments("qa08")
        args.restart_world = True
        with self.assertRaises(prep.auth.CheckFailure): live.validate_qualification_arguments(args)
        args.restart_world = False
        args.qa08_provisioning_budget = None
        with self.assertRaisesRegex(prep.auth.CheckFailure, "qa08-explicit-normal-budget"):
            live.validate_qualification_arguments(args)

    def test_default_cli_stays_in01(self):
        with patch("sys.argv", ["world_in01_live.py", "--bin-dir", "/fixture", "--concert-bin", "/fixture/concert", "--report", "/fixture/report"]), patch.object(live, "run", return_value=0) as execute:
            self.assertEqual(live.main(), 0)
        self.assertEqual(execute.call_args.args[0].qualification_profile, "in01")

    def test_reused_binary_release_fence_rejects_map6_before_resource_work(self):
        bootstrap = configuration()["bootstrap"]
        bootstrap["map_manifest"].update(map_id="mcr_reference_scene_r3", map_revision=4)
        bootstrap["avatar_manifest"].update(manifest_id="mcr_selected_static_20261103", manifest_version=1)
        with patch.object(Path, "read_bytes", return_value=json.dumps(bootstrap).encode()), patch.object(prep, "release_inputs") as pins:
            live.qualified_release()
        pins.assert_called_once()
        bootstrap["map_manifest"]["map_revision"] = 6
        with patch.object(Path, "read_bytes", return_value=json.dumps(bootstrap).encode()), patch.object(prep, "release_inputs") as pins:
            with self.assertRaisesRegex(prep.auth.CheckFailure, "qualification-release-pin"): live.qualified_release()
        pins.assert_not_called()

    def test_probe_object_callback_and_oracle_are_not_serialized(self):
        args, config, principals = handoff()
        config = live.qualification_config(args, config, {"event": config["event_id"], "rooms": config["rooms"]}, principals, config["bootstrap"], config["ws_url"])
        result = {"result": "COMPLETED", "server_metrics_and_cleanup_oracle": {"status": "OBSERVED_PRODUCER_HOOK", "memberships": 0, "prepared_transactions": 0}}
        async def probe(received, **kwargs):
            self.assertIs(received, config)
            self.assertEqual(await kwargs["observation_hook"](), {"memberships": 0, "prepared_transactions": 0})
            return copy.deepcopy(result)
        with patch("world_capacity_support.scenarios.run", side_effect=probe), patch.object(live, "membership_observation", return_value={"memberships": 0, "prepared_transactions": 0}), patch.object(prep, "private_json") as persist:
            observed = live.qualification_run(config, NS(cluster=object()))
        self.assertEqual(observed["result"], "COMPLETED")
        persist.assert_not_called()
        self.assertNotIn(principals[0][2], json.dumps(observed))
        result["server_metrics_and_cleanup_oracle"]["memberships"] = 1
        with patch("world_capacity_support.scenarios.run", new=AsyncMock(return_value=result)):
            self.assertEqual(live.qualification_run(config, NS(cluster=object()))["failure"], "producer-cleanup-oracle")

    def test_restart_only_uses_direct_child_and_fresh_owned_port_reservations(self):
        child = NS(process=NS(pid=123), stop=Mock(return_value={"exit_code": 0, "forced": False}))
        replacement = NS(process=NS(pid=124), wait_listeners=Mock(), stop=Mock())
        ports = Mock()
        names = ["concert-service:grpc", "concert-service:metrics", "concert-service:world"]
        stack = NS(cluster=NS(assert_owned=Mock()), ports=NS(ports=dict(zip(names, [31001, 31002, 31003]))))
        with patch.object(live, "RestartPorts", return_value=ports), patch.object(live.dev.runtime, "Child", return_value=replacement), patch.object(live.dev.runtime, "http_probe", return_value=(200, b'')), patch.object(live.time, "sleep") as wait:
            records = []
            actual, env = live.restart_owned_concert(stack, child, {"CONCERT_WORLD_OWNER_ID": "old"}, Path("/not-executed"), names, records)
        self.assertIs(actual, replacement)
        self.assertNotEqual(env["CONCERT_WORLD_OWNER_ID"], "old")
        self.assertEqual(ports.reserve.call_count, 3)
        ports.close.assert_called_once()
        self.assertEqual(records[0]["after_pid"], 124)
        wait.assert_called_once_with(5.1)
        self.assertIn("natural_owner_lease_wait_seconds", records[0])
        child.stop.return_value = {"exit_code": -9, "forced": True}
        with patch.object(live.dev.runtime, "Child") as spawn:
            with self.assertRaises(prep.auth.CheckFailure): live.restart_owned_concert(stack, child, {}, None, names, [])
        spawn.assert_not_called()

    def test_restart_reservation_is_owned_loopback_and_never_reuseport(self):
        handle = Mock()
        with patch.object(live.socket, "socket", return_value=handle):
            ports = live.RestartPorts()
            self.assertEqual(ports.reserve("concert-service:world", 31003), 31003)
            handle.bind.assert_called_once_with(("127.0.0.1", 31003))
            handle.setsockopt.assert_called_once_with(live.socket.SOL_SOCKET, live.socket.SO_REUSEADDR, 1)
            with self.assertRaises(prep.auth.CheckFailure): ports.reserve("concert-service:world", 31003)
            ports.close()
        handle.close.assert_called_once()

    def test_failed_restart_retains_owned_stop_and_only_public_failure_labels(self):
        child = NS(process=NS(pid=123), stop=Mock(return_value={"exit_code": 0, "forced": False}))
        failure = live.dev.SmokeError("concert-service:startup",
            "World startup replay unavailable SECRET-TOKEN https://private.invalid")
        replacement = NS(process=NS(pid=124), wait_listeners=Mock(side_effect=failure),
                         stop=Mock(return_value={"exit_code": 1, "unexpected_exit": True}))
        names = ["concert-service:grpc", "concert-service:metrics", "concert-service:world"]
        stack = NS(cluster=NS(assert_owned=Mock()), ports=NS(ports=dict(zip(names, [31001,31002,31003]))))
        records = []
        with patch.object(live, "RestartPorts"), patch.object(live.dev.runtime, "Child", return_value=replacement), patch.object(live.time, "sleep"):
            with self.assertRaises(live.dev.SmokeError):
                live.restart_owned_concert(stack, child, {}, Path("/not-executed"), names, records)
        self.assertEqual(records[0]["stop"]["exit_code"], 0)
        self.assertEqual(records[0]["replacement_failure"]["public_source_labels"], ["world-startup-replay"])
        self.assertEqual(records[0]["failed_replacement_cleanup"]["exit_code"], 1)
        self.assertNotIn("SECRET", json.dumps(records))
        self.assertNotIn("private.invalid", json.dumps(records))

    def test_original_accept_cutoffs_survive_delayed_start_and_retry(self):
        args = arguments()
        with tempfile.TemporaryDirectory() as directory:
            args.report = Path(directory) / "report.json"
            now = time.time()
            args.allocation_work_deadline_epoch = now + 10
            args.allocation_release_deadline_epoch = now + 250
            guard = live.QualificationGuard(args, NS(cluster=None))
            self.assertLessEqual(guard.deadline - guard.started_monotonic, 10)
            self.assertLessEqual(guard.hard_deadline_at.timestamp(), now + 250.001)
            self.assertGreaterEqual(guard.hard_deadline - guard.deadline, 240)
            args.allocation_work_deadline_epoch = now - 1
            with self.assertRaisesRegex(Failure, "absolute-work-cutoff"):
                live.validate_qualification_arguments(args)
            args.allocation_work_deadline_epoch = now + 20
            with self.assertRaisesRegex(prep.auth.CheckFailure, "absolute-cleanup-reserve"):
                live.validate_qualification_arguments(args)

    def test_guard_counts_carry_outputs_and_deadline_without_starting_thread(self):
        args = arguments()
        with tempfile.TemporaryDirectory() as directory, patch.object(live.shutil, "disk_usage", return_value=NS(free=20*1024**3)), patch.object(live.subprocess, "run", return_value=NS(stdout="")):
            args.report = Path(directory) / "report.json"
            guard = live.QualificationGuard(args, NS(cluster=None))
            guard.check()
            self.assertGreaterEqual(guard.maximum, args.retained_carry_bytes)
            self.assertIsNone(guard.thread)
            guard.limit = 1
            with self.assertRaisesRegex(prep.auth.CheckFailure, "qualification-whole-byte-cap"): guard.check()
            guard.limit = args.whole_byte_limit
            guard.deadline = 0
            with self.assertRaisesRegex(prep.auth.CheckFailure, "qualification-work-deadline"): guard.check()
            self.assertEqual(guard.close()["cleanup_reserve_seconds"], 240)

    def test_offer_floors_and_absolute_phase_cutoffs_before_children(self):
        args = arguments()
        with tempfile.TemporaryDirectory() as directory, patch.object(live.subprocess, "run", return_value=NS(stdout="")):
            args.report = Path(directory) / "report.json"
            guard = live.QualificationGuard(args, NS(cluster=None))
            with patch.object(live.shutil, "disk_usage", return_value=NS(free=15*1024**3)):
                with self.assertRaisesRegex(prep.auth.CheckFailure, "start-free-space"): guard.start()
            self.assertIsNone(guard.thread)
            with patch.object(live.shutil, "disk_usage", return_value=NS(free=7*1024**3)):
                with self.assertRaisesRegex(prep.auth.CheckFailure, "free-space"): guard.check()
            guard.phase("fixture-phase", 10)
            self.assertLessEqual(guard.phase_deadline, guard.deadline)
            evidence = guard.close()
            self.assertIn("+00:00", evidence["hard_deadline_at"])
            self.assertEqual(evidence["phase_cutoffs"][0]["name"], "fixture-phase")

    def test_process_guard_counts_only_producer_and_owned_descendants(self):
        args = arguments()
        with tempfile.TemporaryDirectory() as directory, patch.object(live.os, "getpid", return_value=100):
            args.report = Path(directory) / "report.json"
            guard = live.QualificationGuard(args, NS(cluster=None))
            with patch.object(live.subprocess, "run", return_value=NS(stdout="100 1\n200 100\n300 200\n999 1\n")):
                guard.process_check()
            self.assertEqual(guard.process_peak, 3)
            with patch.object(live.subprocess, "run", return_value=NS(stdout="\n".join(f"{n} 100" for n in range(200,233)))):
                with self.assertRaisesRegex(prep.auth.CheckFailure, "process-cap"): guard.process_check()
            self.assertEqual(guard.process_limit, 32)
            args.max_owned_processes = 80  # Fixture offered cap only; never runtime authorization.
            configured = live.QualificationGuard(args, NS(cluster=None))
            self.assertEqual(configured.close()["owned_process_cap"], 80)
            args.max_owned_processes = 129
            with self.assertRaises(Failure): live.validate_qualification_arguments(args)

    def test_runtime_migration_overlay_keeps_binary_source_and_exact_ledger_guard(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve() / "backend/services/post-service/migrations"
            root.mkdir(parents=True)
            for n in range(1,32): (root / f"{n:04d}_fixture.sql").write_text(f"fixture migration {n}")
            original = {"post-service": live.dev.migration.migration_manifest(root.parents[2], "post-service"),
                        "auth-service": {1: "b"*96}}
            raw = live.subprocess.run(["/usr/bin/git", "show", live.POST_RUNTIME_MIGRATION_SOURCE+":"+live.POST_RUNTIME_MIGRATION_PATH],
                cwd=live.ROOT, capture_output=True, check=True).stdout  # Public source metadata only.
            (root / "0032_fixture.sql").write_bytes(raw)
            concert = root.parents[1] / "concert-service/migrations"
            concert.mkdir(parents=True)
            for n in range(1,38): (concert / f"{n:04d}_fixture.sql").write_text(f"concert fixture {n}")
            original["concert-service"] = live.dev.migration.migration_manifest(concert.parents[2], "concert-service")
            raw49 = live.subprocess.run(["/usr/bin/git", "show", live.CONCERT_RUNTIME_MIGRATION_SOURCE+":"+live.CONCERT_RUNTIME_MIGRATION_PATH],
                cwd=live.ROOT, capture_output=True, check=True).stdout
            (concert / "0049_fixture.sql").write_bytes(raw49)
            raw50 = live.subprocess.run(["/usr/bin/git", "show", live.CONCERT_NOTIFICATION_MIGRATION_SOURCE+":"+live.CONCERT_NOTIFICATION_MIGRATION_PATH],
                cwd=live.ROOT, capture_output=True, check=True).stdout
            (concert / "0050_fixture.sql").write_bytes(raw50)
            raw51 = live.subprocess.run(["/usr/bin/git", "show", live.CONCERT_SOCIAL_MIGRATION_SOURCE+":"+live.CONCERT_SOCIAL_MIGRATION_PATH],
                cwd=live.ROOT, capture_output=True, check=True).stdout
            (concert / "0051_fixture.sql").write_bytes(raw51)
            with patch.object(live, "POST_RUNTIME_MIGRATION_ROOT", root):
                bound, receipt = live.qualified_runtime_migrations(original)
                (concert / "0052_fixture.sql").write_text("unqualified future runtime input")
                with self.assertRaisesRegex(prep.auth.CheckFailure, "concert-runtime-manifest"):
                    live.qualified_runtime_migrations(original)
                (concert / "0052_fixture.sql").unlink()
                (root / "0001_fixture.sql").write_text("changed runtime input")
                with self.assertRaisesRegex(prep.auth.CheckFailure, "runtime-migration-manifest"):
                    live.qualified_runtime_migrations(original)
                (root / "0001_fixture.sql").write_text("fixture migration 1")
                (concert / "0001_fixture.sql").write_text("changed concert runtime input")
                with self.assertRaisesRegex(prep.auth.CheckFailure, "concert-runtime-manifest"):
                    live.qualified_runtime_migrations(original)
        self.assertNotIn(32, original["post-service"])
        self.assertEqual(len(bound["post-service"]), 32)
        self.assertEqual(bound["post-service"][32], live.POST_RUNTIME_MIGRATION_SHA384)
        self.assertEqual(bound["post-service"][31], original["post-service"][31])
        self.assertEqual(bound["auth-service"], original["auth-service"])
        self.assertEqual(receipt["compiled_source_unchanged"], "d0f063d18a0549935f80a536077c30962d89f9e9")
        self.assertEqual(receipt["runtime_input_source_revision"], live.POST_RUNTIME_MIGRATION_SOURCE)
        self.assertEqual(receipt["generic_ledger_check"], "UNCHANGED_EXACT_MATCH")
        self.assertEqual(len(bound["concert-service"]), 40)
        self.assertNotIn(49, original["concert-service"])
        self.assertNotIn(50, original["concert-service"])
        self.assertNotIn(51, original["concert-service"])
        self.assertEqual(bound["concert-service"][51], live.CONCERT_SOCIAL_MIGRATION_SHA384)
        self.assertEqual(receipt["concert_runtime_input"]["runtime_input_source_revision"], live.CONCERT_RUNTIME_MIGRATION_SOURCE)
        self.assertEqual(receipt["concert_runtime_input"]["notification_runtime_input"]["source_revision"], live.CONCERT_NOTIFICATION_MIGRATION_SOURCE)
        self.assertEqual(receipt["concert_runtime_input"]["social_runtime_input"]["source_revision"], live.CONCERT_SOCIAL_MIGRATION_SOURCE)
        with self.assertRaisesRegex(prep.auth.CheckFailure, "runtime-versions"):
            live.qualified_runtime_migrations({"post-service": {**bound["post-service"], 33: "a"*96}})

    def test_runtime_migration_overlay_refuses_wrong_public_source(self):
        with patch.object(live.subprocess, "run", return_value=NS(stdout=b"unqualified SQL")):
            with self.assertRaisesRegex(prep.auth.CheckFailure, "runtime-migration-source"):
                live.qualified_runtime_migrations({"post-service": {n: "a"*96 for n in range(1,32)}})

    def test_retained_snapshot_is_actual_validated_private_and_never_flag_flipped(self):
        bootstrap = json.loads((prep.RELEASE / "bootstrap.json").read_bytes())
        bootstrap["capabilities"]["world"] = {"available": True, "reason": None}  # Synthetic positive fixture only.
        raw = json.dumps(bootstrap, indent=2).encode() + b'\n'
        principal = (bootstrap["participant"]["user_id"], None, "PRIVATE-TEST-TOKEN")
        with tempfile.TemporaryDirectory() as directory, patch.object(live, "public_json", return_value=raw):
            metadata = live.retain_public_projection(None, {}, bootstrap["event"]["event_id"], principal, Path(directory))
            path = Path(directory) / metadata["filename"]
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            projection = json.loads(path.read_bytes())
            self.assertEqual(projection["fidelity"], "NOT_BYTE_EXACT_FULL_C03")
            self.assertEqual(projection["map_descriptor"]["content_hash"], bootstrap["map_manifest"]["content_hash"])
            self.assertFalse(projection["resources_are_complete_manifests"])
            self.assertNotIn("participant", projection)
            self.assertNotIn("avatar", projection)
            self.assertFalse(metadata["full_body_retained_or_hashed"])
            self.assertNotEqual(path.read_bytes(), raw)
            self.assertNotIn(principal[0], path.read_text())
            self.assertIsNone(re.search(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}", path.read_text()))
            self.assertNotIn(principal[2], path.read_text())
            self.assertNotIn(principal[0], json.dumps(metadata))
        bootstrap["capabilities"]["world"] = {"available": False, "reason": "FEATURE_DISABLED"}
        with patch.object(live, "public_json", return_value=json.dumps(bootstrap).encode()), patch.object(live.dev.runtime, "private_file") as persist:
            with self.assertRaises(Failure): live.retain_public_projection(None, {}, bootstrap["event"]["event_id"], principal, Path("/not-written"))
        persist.assert_not_called()


class StagedAdmissionTests(unittest.IsolatedAsyncioTestCase):
    async def test_preissue_ticket_before_last_slot_then_switch_full(self):
        config = configuration()
        config["rooms"][0]["capacity"] = config["rooms"][1]["capacity"] = 1
        config["principals"][1]["room_index"] = 1
        contracts = Contracts()
        network = FixtureNetwork(config, contracts)
        original = network.http
        seen = []
        async def http(path, token, payload=None):
            if payload and token == config["principals"][0]["bearer"] and payload["room_id"] == config["rooms"][1]["room_id"]:
                seen.append(sum(s.room == payload["room_id"] and not s.closed for s in network.sockets))
            return await original(path, token, payload)
        network.http = http
        runner = Runner(config, False, network, contracts)
        runner.capacity = True  # Two-client control-order fixture, not a 200-socket test.
        try:
            await runner.prepare()
            self.assertEqual(seen, [0])
            await runner.switch(runner.clients[0], 1, True, auth=runner.full_room_auth)
            self.assertEqual(runner.clients[0].state.rejection, "ROOM_FULL")
        finally:
            for client in runner.clients: await client.stop()


if __name__ == "__main__": unittest.main()
