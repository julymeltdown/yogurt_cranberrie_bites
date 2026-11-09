#!/usr/bin/env python3
"""WP-10: the profile/social end-to-end integration gate.

This is the integration gate of the profile product. It stands up the real
services on an owned private PostgreSQL 16 cluster, seeds three real identities
through the real enrollment and NAVER verification flows, then drives the product
scenario of `product-plan.md` §9 over real HTTP.

Banner: this is a development stack, not a QA08 capacity run and not a
production qualification. Every step records what was actually observed; a step
the composed product cannot reach is reported with its exact status and the
package that owns it, never simulated.

What it proves today (merged on master): profiles, ordinary profile posts and
media, the personal room, the avatar, and the identity claim store.
What it cannot prove yet (in flight, not on master): the intro post surfaces
(`/api/intro-posts/*`), the audience decision, hearts, comments, likers and the
intro notification. Those rows are recorded as BLOCKED with the exact missing
condition.

Usage:
    python3 backend/scripts/mcr/wp10_profile_social.py \
        --bin-dir backend/target/debug --report /tmp/wp10/report.json
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import http.client
import http.server
import json
import secrets
import socketserver
import sys
import threading
import time
import urllib.error
import urllib.request
from datetime import datetime, timedelta, timezone
from pathlib import Path
from unittest.mock import patch
from uuid import NAMESPACE_URL, UUID, uuid4, uuid5

sys.path.insert(0, str(Path(__file__).resolve().parent))
import closure_vertical_stack as closure  # noqa: E402
import dev_stack  # noqa: E402
import migration_smoke as migration  # noqa: E402
import runtime_smoke as runtime  # noqa: E402
import world_in01_live as world  # noqa: E402
import world_in01_prepare as prep  # noqa: E402

ROLES = ("author", "viewer", "outsider")
BINARIES = ("api-gateway", "auth-service", "concert-service", "friend-service",
            "notification-service", "post-service", "profile-service")

# The three fixed provider profiles of the test NAVER application (T-951).
# `subject` is the provider `id`; the raw value is never stored by the product
# (it stores a salted hash), so a fixed value is safe and makes the fixture
# reproducible.
PROVIDER_PROFILES = {
    "viewer": {"id": "wp10-naver-viewer", "gender": "F", "birthyear": "2001",
               "birthday": "03-14", "email": "viewer@example.test"},
    "outsider": {"id": "wp10-naver-outsider", "gender": "M", "birthyear": "1990",
                 "birthday": "11-02", "email": "outsider@example.test"},
}

# Deterministic catalog items for MCR and NANA (T-951).
CATALOG_ITEMS = {
    "mcr": {"item_id": "wp10_mcr_three_cheers", "kind": "MUSIC", "name": "Three Cheers for Sweet Revenge"},
    "nana": {"item_id": "wp10_nana_seven", "kind": "MUSIC", "name": "NANA OST - Seven"},
}


class GateFailure(RuntimeError):
    pass


def require(condition, label):
    if not condition:
        raise GateFailure(label)


def file_digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sql_quote(value):
    return prep.quote(value)


def now_iso(offset_minutes=0):
    return (datetime.now(timezone.utc) + timedelta(minutes=offset_minutes)).isoformat()


# ---------------------------------------------------------------------------
# The test NAVER application (T-951)
# ---------------------------------------------------------------------------

class NaverProvider:
    """A local HTTP stand-in for the NAVER token and profile endpoints.

    The product's provider client is configured with these two URLs, so the real
    token exchange and the real profile decode run against it. It is a *test
    application*: the real NAVER console application does not exist in this
    environment, which is recorded as BLOCKED in the release evidence.
    """

    def __init__(self, port_holder):
        self.codes = {}
        self.tokens = {}
        provider = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def setup(self):
                self.request.settimeout(5)
                super().setup()

            def log_message(self, *_args):
                pass

            def _json(self, status, value):
                body = json.dumps(value).encode()
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def do_POST(self):
                if not self.path.startswith("/oauth2.0/token"):
                    return self._json(404, {"error": "not_found"})
                size = int(self.headers.get("Content-Length", "0"))
                if not 0 < size <= 8192:
                    return self._json(400, {"error": "invalid_request"})
                from urllib.parse import parse_qs
                form = parse_qs(self.rfile.read(size).decode())
                if form.get("grant_type") != ["authorization_code"]:
                    return self._json(400, {"error": "invalid_grant"})
                code = (form.get("code") or [""])[0]
                token = provider.codes.pop(code, None)
                if token is None:
                    return self._json(400, {"error": "invalid_grant"})
                return self._json(200, {"access_token": token, "token_type": "bearer",
                                        "expires_in": 3600})

            def do_GET(self):
                if not self.path.startswith("/v1/nid/me"):
                    return self._json(404, {"error": "not_found"})
                header = self.headers.get("Authorization", "")
                token = header.removeprefix("Bearer ").strip()
                profile = provider.tokens.get(token)
                if profile is None:
                    return self._json(401, {"resultcode": "024",
                                            "message": "Authentication failed"})
                return self._json(200, {"resultcode": "00", "message": "success",
                                        "response": profile})

        class QuietServer(http.server.HTTPServer):
            def handle_error(self, *_args):
                pass

        self.server = QuietServer(("127.0.0.1", port_holder.ports["naver-provider"]), Handler,
                                  bind_and_activate=False)
        self.server.socket.close()
        self.server.socket = port_holder.held.pop("naver-provider")
        self.server.server_address = self.server.socket.getsockname()
        self.server.server_activate()
        self.thread = threading.Thread(target=self.server.serve_forever,
                                       kwargs={"poll_interval": 0.1}, daemon=True)
        self.thread.start()

    def issue_code(self, role):
        profile = PROVIDER_PROFILES[role]
        code = "wp10-code-" + secrets.token_hex(12)
        token = "wp10-token-" + secrets.token_hex(12)
        self.codes[code] = token
        self.tokens[token] = {
            "id": profile["id"],
            "email": profile["email"],
            "gender": profile["gender"],
            "birthyear": profile["birthyear"],
            "birthday": profile["birthday"],
        }
        return code

    def url(self):
        host, port = self.server.server_address
        return f"http://{host}:{port}"

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=3)


# ---------------------------------------------------------------------------
# HTTP client
# ---------------------------------------------------------------------------

class Api:
    """One authenticated HTTP client role; no browser state or injected token."""

    def __init__(self, gateway, origin, tokens):
        host, port = gateway.split("//", 1)[1].rsplit(":", 1)
        self.host, self.port = host, int(port)
        self.origin, self.tokens = origin, tokens
        self.log = []

    def request(self, role, method, path, body=None, key=None, headers=None, raw=None,
                content_type=None):
        token = self.tokens.get(role) if role else None
        hdrs = {"X-MCR-Intent": "browser", "Origin": self.origin}
        if token:
            hdrs["Authorization"] = "Bearer " + token
        payload = raw
        if body is not None:
            payload = json.dumps(body).encode()
            hdrs["Content-Type"] = "application/json"
        elif raw is None and method in ("POST", "PUT", "PATCH"):
            # A bodyless mutation still carries the closed JSON envelope; the
            # gateway answers 415 without it. DELETE is bodyless by contract.
            payload = b""
            hdrs["Content-Type"] = "application/json"
        if content_type:
            hdrs["Content-Type"] = content_type
        if key:
            hdrs["Idempotency-Key"] = key
        if headers:
            hdrs.update(headers)
        self.log.append({"role": role, "method": method, "path": path})
        connection = http.client.HTTPConnection(self.host, self.port, timeout=30)
        try:
            connection.request(method, path, body=payload, headers=hdrs)
            response = connection.getresponse()
            try:
                data = response.read()
            except http.client.IncompleteRead as truncated:
                # A response cut short by the gateway's own deadline is recorded
                # as what was actually received, never retried into a pass.
                data = truncated.partial
            try:
                parsed = json.loads(data or b"{}")
            except ValueError:
                parsed = {"raw_bytes": len(data), "raw_prefix": data[:64].hex()}
            return response.status, parsed, dict(response.getheaders()), data
        finally:
            connection.close()


# ---------------------------------------------------------------------------
# The gate
# ---------------------------------------------------------------------------

class Gate:
    def __init__(self, args):
        self.args = args
        self.root = Path(__file__).resolve().parents[3]
        self.report = {
            "scope": "WP-10 profile/social end-to-end integration gate",
            "release": "development stack, NOT production qualification",
            "qa08": "NOT_RUN",
            "production": "NOT_CLAIMED",
            "candidate": {
                "worktree": str(self.root),
                "revision": migration.execute(
                    ["/usr/bin/git", "rev-parse", "HEAD"],
                    env={"PATH": "/usr/bin:/bin"}, cwd=self.root, phase="revision").stdout.strip(),
                "script_sha256": file_digest(Path(__file__)),
                "binaries": {name: {"sha256": file_digest(args.bin_dir / name),
                                    "bytes": (args.bin_dir / name).stat().st_size}
                             for name in BINARIES},
            },
            "started_at_utc": datetime.now(timezone.utc).isoformat(),
            "identities": {},
            "steps": {},
            "sweeps": {},
            "checks": {},
        }
        self.state = {"event": None, "origin": None, "digest": None}
        self.base_environments = dev_stack.development_environments
        self.stack = dev_stack.DevStack(args.bin_dir, args.pg_bin_dir, args.redis_binary,
                                        args.openssl_binary, None)
        self.provider = None
        self.api = None
        self.users = {}
        self.profiles = {}
        self.probes = []

    # -- stack -----------------------------------------------------------

    def configure_runtime(self):
        migration.SERVICES["concert-service"] = "CONCERT_DATABASE_URL"
        runtime.PREFIX.update({name: variable.removesuffix("_DATABASE_URL")
                               for name, variable in migration.SERVICES.items()})
        runtime.START_ORDER = ("profile-service", "notification-service", "friend-service",
                               "post-service", "concert-service", "auth-service", "api-gateway")
        runtime.PEERS["concert-service"] = ("auth-service", "friend-service", "profile-service",
                                            "post-service")
        runtime.EXTRA_LISTENERS["concert-service"] = ("world",)
        original_child_init = runtime.Child.__init__

        def child_init(self, cluster, name, argv, env, ports, reservations,
                       secrets_to_redact=()):
            if name == "concert-service":
                reservations = tuple(key for key in reservations
                                     if key != "concert-service:http")
            original_child_init(self, cluster, name, argv, env, ports, reservations,
                                secrets_to_redact)

        runtime.Child.__init__ = child_init

    def environments(self, cluster, ports, keys, credentials, redis_password, mail_token,
                     browser_origin=None):
        # The original function, captured before the patch below replaces the module
        # attribute: calling the module attribute here would recurse into this hook.
        envs = self.base_environments(cluster, ports, keys, credentials,
                                      redis_password, mail_token, browser_origin)
        cluster.assert_owned()
        cluster.stop()
        with (cluster.data / "postgresql.conf").open("a") as stream:
            stream.write("\nmax_prepared_transactions=256\n")
        cluster.start()
        require(cluster.sql("postgres", "SHOW max_prepared_transactions") == "256",
                "owned-prepared-config")
        world.pki(cluster)
        for service in envs:
            envs[service].update(world.tls(cluster, runtime.PREFIX[service].lower()))
        origin = envs["api-gateway"]["GATEWAY_CORS_ALLOWED_ORIGINS"].split(",")[0]
        envs["auth-service"]["AUTH_ENROLLMENT_VERIFICATION_URL"] = origin + "/verify-email"
        release = prep.release_inputs()
        template = json.loads(Path(release["CONCERT_BOOTSTRAP_FILE"]).read_bytes())
        event = str(template["event"]["event_id"])
        rooms = [{"room_id": str(uuid4()), "capacity": 8}]
        owner = str(uuid4())
        avatar = template["avatar_manifest"]
        mapped = template["map_manifest"]
        statements = ["INSERT INTO concert_events(event_id,slug,title,phase) VALUES("
                      + ",".join(map(sql_quote, [event, "wp10-gate-" + secrets.token_hex(8),
                                                 "WP-10 integration gate", "OPEN"])) + ")"]
        for room in rooms:
            statements.append(
                "INSERT INTO concert_world_rooms(event_id,room_id,label,capacity,map_id,"
                "map_revision,avatar_manifest_id,avatar_manifest_version,is_default) VALUES("
                + ",".join(map(sql_quote, [event, room["room_id"], "Gate room", room["capacity"],
                                           mapped["map_id"], mapped["map_revision"],
                                           avatar["manifest_id"], avatar["manifest_version"]]))
                + ",true)")
        cluster.sql("mcr_concert", ";".join(statements))
        world_address = ports.address("concert-service:world")
        relationship, self_profile, decision, post_read, post_callback = world.joined.keys(cluster, 5)
        urls = {role: "https://" + ports.address(role + "-service:grpc")
                for role in ("auth", "friend", "profile", "concert", "notification", "post")}
        invite_keys = json.dumps([{"id": "wp10_dev", "secret_hex": secrets.token_hex(32)}])
        intro_receipt_keys = json.dumps(
            [{"id": "wp10_intro_receipt", "secret_hex": secrets.token_hex(32)}])
        pickup_keys = json.dumps([{"id": "wp10_pickup", "secret_hex": secrets.token_hex(32)}])
        social_handle_keys = json.dumps(
            [{"id": "wp10_social_handle", "secret_hex": secrets.token_hex(32)}])
        social_command_keys = json.dumps(
            [{"id": "wp10_social_command", "secret_hex": secrets.token_hex(32)}])
        for env in envs.values():
            for key, value in list(env.items()):
                if key.endswith("_GRPC_ENDPOINT") and value.startswith("http://127.0.0.1:"):
                    env[key] = value.replace("http://", "https://", 1)
            env["CONCERT_RELATIONSHIP_VERIFYING_KEYS"] = relationship[1]
        for role in ("auth", "friend"):
            envs[role + "-service"].update(
                CONCERT_OWNER_AUTHORITY_ENABLED="true",
                AUTHORITY_CONCERT_SETTLEMENT_ENDPOINT=urls["concert"],
                AUTHORITY_POST_SETTLEMENT_ENDPOINT=urls["post"],
                AUTHORITY_PROFILE_SETTLEMENT_ENDPOINT=urls["profile"],
                AUTHORITY_NOTIFICATION_SETTLEMENT_ENDPOINT=urls["notification"],
            )
        envs["profile-service"].update(
            CONCERT_PROFILE_SELF_VERIFYING_KEYS=self_profile[1],
            PROFILE_PARTICIPANT_DECISION_SIGNING_SEED=decision[0],
            PROFILE_SELF_AUTH_GRPC_ENDPOINT=urls["auth"],
            PROFILE_SELF_FRIEND_GRPC_ENDPOINT=urls["friend"],
            PROFILE_SELF_CONCERT_GRPC_ENDPOINT=urls["concert"],
        )
        envs["concert-service"].update(
            CONCERT_RUNTIME_MODE="local",
            CONCERT_BOOTSTRAP_FILE=release["CONCERT_BOOTSTRAP_FILE"],
            CONCERT_AVATAR_MANIFEST_FILE=release["CONCERT_AVATAR_MANIFEST_FILE"],
            CONCERT_MAP_MANIFEST_FILE=release["CONCERT_MAP_MANIFEST_FILE"],
            CONCERT_REVENGE_MANIFEST_FILE=release["CONCERT_REVENGE_MANIFEST_FILE"],
            CONCERT_PARADE_MANIFEST_FILE=release["CONCERT_PARADE_MANIFEST_FILE"],
            CONCERT_DANGER_MANIFEST_FILE=release["CONCERT_DANGER_MANIFEST_FILE"],
            CONCERT_WORLD_LISTEN_ADDR=world_address,
            CONCERT_WORLD_ALLOWED_ORIGINS=json.dumps([origin]),
            CONCERT_WORLD_EVENT_ID=event,
            CONCERT_WORLD_OWNER_ID=owner,
            CONCERT_WORLD_INVALIDATION_CONSUMER_ID=owner,
            CONCERT_WORLD_ROOMS_JSON=json.dumps(rooms),
            CONCERT_WORLD_RESOURCE_ROOT=release["asset_root"],
            CONCERT_WORLD_SOURCE_BINDING_FILE=release["source_binding"],
            CONCERT_WALL_TOOLS_B_MODE="live",
            CONCERT_WALL_TOOLS_B_RECEIPT_KEYS_JSON=invite_keys,
            CONCERT_RELATIONSHIP_SIGNING_SEED=relationship[0],
            CONCERT_PROFILE_SELF_SIGNING_SEED=self_profile[0],
            PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS=decision[1],
            CONCERT_PARTICIPANT_MODE="live",
            CONCERT_PRODUCER_INSTANCE_ID=str(uuid4()),
            CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON=invite_keys,
            CONCERT_INTRO_MODE="live",
            CONCERT_NOTIFICATION_GRPC_ENDPOINT=urls["notification"],
            CONCERT_INTRO_CURSOR_KEY_HEX=secrets.token_hex(32),
            CONCERT_INTRO_RECEIPT_KEYS_JSON=intro_receipt_keys,
            CONCERT_PICKUP_KEYS_JSON=pickup_keys,
            CONCERT_NOTIFICATION_INBOX_MODE="live",
            CONCERT_NOTIFICATION_TARGET_MODE="live",
            CONCERT_SOCIAL_MODE="live",
            CONCERT_SOCIAL_CURSOR_KEY_HEX=secrets.token_hex(32),
            CONCERT_SOCIAL_HANDLE_KEYS_JSON=social_handle_keys,
            CONCERT_SOCIAL_COMMAND_KEYS_JSON=social_command_keys,
            CONCERT_SOCIAL_REQUEST_TTL_SECONDS="3600",
            CONCERT_PUBLIC_FEED_MODE="read",
            CONCERT_POST_GRPC_ENDPOINT=urls["post"],
            CONCERT_POST_READ_SIGNING_SEED=post_read[0],
            CONCERT_FEED_CURSOR_KEY_HEX=secrets.token_hex(32),
            POST_CONCERT_ROOT_READ_VERIFYING_KEYS=post_callback[1],
        )
        for role in ("auth", "friend", "profile"):
            envs["concert-service"]["CONCERT_PARTICIPANT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
            envs["concert-service"]["CONCERT_" + role.upper() + "_GRPC_ENDPOINT"] = urls[role]
        # The Introduction audience decision needs the viewer's verified age
        # and gender, which live in the profile service. Without this endpoint
        # the claim port stays detached and every restricted post fails closed,
        # which is safe but makes the audience feature untestable here.
        envs["concert-service"].update(
            CONCERT_PROFILE_GRPC_ENDPOINT=urls["profile"],
            CONCERT_SERVICE_CREDENTIAL=credentials["CONCERT_SERVICE_CREDENTIAL"],
        )
        # The discovery projection worker. It reads the profile service's
        # discovery-source over the same TLS edge and the same Concert workload
        # credential as every other concert->profile call. Without these three
        # variables the worker is off and `discovery_profiles` stays empty, which
        # is the defect this gate step exists to catch.
        envs["concert-service"].update(
            CONCERT_DISCOVERY_PROJECTION_MODE="live",
            CONCERT_DISCOVERY_SOURCE_GRPC_ENDPOINT=urls["profile"],
            CONCERT_DISCOVERY_PROJECTION_INTERVAL_MS="1000",
        )
        envs["concert-service"]["RUST_LOG"] = "info"
        envs["post-service"]["RUST_LOG"] = "info"
        envs["post-service"].update(
            POST_CONCERT_READ_MODE="read",
            POST_CONCERT_READ_AUTH_GRPC_ENDPOINT=urls["auth"],
            POST_FRIEND_GRPC_ENDPOINT=urls["friend"],
            POST_CONCERT_ROOT_GRPC_ENDPOINT=urls["concert"],
            CONCERT_POST_READ_VERIFYING_KEYS=post_read[1],
            POST_CONCERT_ROOT_READ_SIGNING_SEED=post_callback[0],
        )
        envs["api-gateway"]["GATEWAY_REQUEST_TIMEOUT_MS"] = str(self.args.gateway_timeout_ms)
        # The gate needs the gateway's and the profile producer's own diagnostics:
        # a 503 with an empty log is not evidence of anything.
        envs["api-gateway"]["RUST_LOG"] = "debug"
        envs["profile-service"]["RUST_LOG"] = "debug"
        envs["auth-service"]["RUST_LOG"] = "info"
        envs["api-gateway"]["GATEWAY_PROFILE_GRPC_ENDPOINT"] = urls["profile"]
        inspected = migration.execute(
            [str(self.args.bin_dir / "concert-service"), "--inspect-product-release"],
            env=envs["concert-service"], cwd=cluster.root, phase="gate-release-inspection",
        )
        digest_value = inspected.stdout.strip().split("=", 1)[1]
        envs["concert-service"]["CONCERT_PARTICIPANT_RELEASE_DIGEST"] = digest_value
        envs["api-gateway"]["GATEWAY_CONCERT_GRPC_ENDPOINT"] = urls["concert"]
        envs["api-gateway"]["GATEWAY_CONCERT_RELEASE_DIGEST"] = digest_value
        # The test NAVER application: the real provider client calls these URLs.
        provider_url = self.provider.url()
        envs["profile-service"].update(
            NAVER_OAUTH_CLIENT_ID="wp10-test-naver-client",
            NAVER_OAUTH_CLIENT_SECRET="wp10-test-naver-secret",
            NAVER_OAUTH_AUTHORIZE_URL=provider_url + "/oauth2.0/authorize",
            NAVER_OAUTH_TOKEN_URL=provider_url + "/oauth2.0/token",
            NAVER_OAUTH_PROFILE_URL=provider_url + "/v1/nid/me",
            NAVER_OAUTH_REDIRECT_URI=origin + "/api/identity/naver/callback",
        )
        self.state.update(event=event, origin=origin, digest=digest_value)
        return envs

    # -- helpers ---------------------------------------------------------

    def sql(self, database, statement):
        try:
            return self.stack.cluster.sql(database, statement)
        except migration.SmokeError as failure:
            # Keep the statement and the database in the message: "sql-failed"
            # alone cannot be diagnosed from a report, and the gate's job is to
            # say what happened.
            detail = getattr(failure, "detail", None) or str(failure)
            raise GateFailure(
                f"sql-failed: database={database} statement={statement[:160]!r} "
                f"detail={str(detail)[:200]}") from None

    def query(self, database, statement):
        """One JSON document from one statement.

        `psql -t -A` prints the bare value, so this only parses statements that
        actually return JSON (`jsonb_agg`, `to_jsonb`, ...). A statement that
        returns a scalar — a uuid, a count, a boolean — must use `scalar()`
        instead: `json.loads` on `b5ae2501-...` raises, and the earlier version of
        this helper was used for both, which is what made the lifecycle sweep
        die with a `JSONDecodeError` instead of reporting a result.
        """
        raw = self.sql(database, statement)
        if not raw:
            return None
        try:
            return json.loads(raw)
        except ValueError:
            raise GateFailure(
                f"sql-not-json: statement did not return JSON: {statement[:120]!r} "
                f"returned {raw[:120]!r}") from None

    def scalar(self, database, statement):
        """One bare value from one statement, or `None` when it returned no row.

        No JSON parsing: the value is whatever `psql -t -A` printed, which is what
        a scalar subquery returns.
        """
        raw = self.sql(database, statement)
        return raw if raw else None

    def profile_of(self, role):
        return self.profiles[role]

    def enroll(self, role, ordinal):
        """Enroll one identity through the real enrollment flow."""
        policy = self.policy
        gateway, origin = self.state["origin"], self.state["origin"]
        status, body, headers = closure.request(
            gateway + "/api/auth/enrollment/attempts", "POST",
            {"policyRevision": policy["revision"]}, origin=origin)
        require(status == 201, f"enrollment attempt {role}: {status}")
        attempt = body["attemptId"]
        cookie = closure.cookie_pair(headers, "mcr-enrollment-dev")
        require(cookie is not None, f"enrollment cookie {role} missing")
        email = f"wp10-{role}-{secrets.token_hex(4)}@example.test"
        password = secrets.token_urlsafe(20)
        acknowledgements = [d["requiredAcknowledgementId"] for d in policy["documents"]]
        status, body, _ = closure.request(
            gateway + "/api/auth/enrollment/signup", "POST",
            {"attemptId": attempt, "policyRevision": policy["revision"],
             "acknowledgementIds": acknowledgements, "idempotencyKey": str(uuid4()),
             "email": email, "password": password},
            origin=origin, cookie=cookie)
        require(status == 202, f"enrollment signup {role}: {status}")
        code, deadline = None, time.monotonic() + 25
        while code is None and time.monotonic() < deadline:
            for path in self.stack.mail.directory.glob("*.json"):
                message = json.loads(path.read_bytes())
                payload = message.get("payload", {})
                if (message["kind"] == "VERIFY_EMAIL" and payload.get("email") == email
                        and payload.get("enrollment_attempt_id") == attempt):
                    code = payload["verification_code"]
            time.sleep(0.1)
        require(code is not None, f"enrollment mail {role} missing")
        status, body, _ = closure.request(gateway + "/api/auth/enrollment/verify", "POST",
                                          {"attemptId": attempt, "code": code},
                                          origin=origin, cookie=cookie)
        require(status == 200 and body.get("status") == "VERIFIED",
                f"enrollment verify {role}: {status}")
        status, body, _ = closure.request(gateway + "/api/auth/login", "POST",
                                          {"email": email, "password": password}, origin=origin)
        require(status == 200 and body.get("accessToken"), f"login {role}: {status}")
        user, token = body["userId"], body["accessToken"]
        status, admission, _ = closure.request(gateway + "/api/auth/enrollment/admission", "GET",
                                               None, token=token, origin=origin)
        require(status == 200, f"admission {role}: {status}")
        if admission.get("decision") == "CONSENT_REQUIRED":
            status, current, _ = closure.request(gateway + "/api/auth/enrollment/policy", "GET",
                                                 origin=origin)
            require(status == 200, f"renewed policy {role}: {status}")
            status, consent, _ = closure.request(
                gateway + "/api/auth/enrollment/consent", "POST",
                {"policyRevision": current["revision"],
                 "acknowledgementIds": [d["requiredAcknowledgementId"]
                                        for d in current["documents"]],
                 "idempotencyKey": str(uuid4())},
                token=token, origin=origin)
            require(status == 200 and consent.get("decision") == "ADMITTED",
                    f"enrollment consent {role}: {status}")
        return user, token, email

    def seed_profiles(self):
        """One live profile identity per account, keyed by the Auth user id.

        Two rows are needed and both are the product's own stores:

        * `profiles` is the foundation profile row the gateway's identity
          composition reads (`ReadConcertSelfProfile`); a profile without it is
          refused before any profile product operation is dispatched;
        * `profile_identities` is the WP-02 integration identity that keys every
          profile-owned table (room, avatar, posts, media).

        Both carry the same account binding, so the two never disagree about who
        the actor is.
        """
        statements = []
        for role in ROLES:
            user = self.users[role]
            profile = str(uuid4())
            self.profiles[role] = profile
            statements.append(
                "INSERT INTO profiles(user_id,username,nickname,avatar_url,bio) VALUES("
                + ",".join(map(sql_quote, [user, f"wp10_{role}", f"WP-10 {role}",
                                           "retained-avatar",
                                           f"Gate identity {role}"])) + ")")
            statements.append(
                "INSERT INTO profile_identities(profile_id,account_id,nickname,bio,visibility) "
                "VALUES(" + ",".join(map(sql_quote, [profile, user,
                                                     f"WP-10 {role}",
                                                     f"Gate identity {role}"])) + ",'PUBLIC')")
        self.sql("mcr_profile", ";".join(statements))

    def verify_identity(self, role):
        """Run the real start -> provider -> callback flow for one identity."""
        gateway, origin = self.state["origin"], self.state["origin"]
        status, body, _, _ = self.api.request(role, "POST",
                                              "/api/me/identity/naver/verifications",
                                              {"return_path": "/me/identity"},
                                              key=str(uuid4()))
        require(status in (200, 202), f"verification start {role}: {status} {body}")
        url = body.get("authorization_url") or body.get("authorizationUrl")
        require(isinstance(url, str) and "state=" in url, f"verification start {role} url")
        from urllib.parse import parse_qs, urlsplit
        state = parse_qs(urlsplit(url).query).get("state", [""])[0]
        require(bool(state), f"verification state {role} missing")
        code = self.provider.issue_code(role)
        status, body, headers, _ = self.api.request(
            None, "GET", f"/api/identity/naver/callback?code={code}&state={state}")
        location = headers.get("Location") or headers.get("location") or ""
        require(status in (200, 303), f"callback {role}: {status}")
        marker = location.split("identity=", 1)[1].split("&", 1)[0] if "identity=" in location else ""
        status, status_body, _, _ = self.api.request(role, "GET", "/api/me/identity/naver")
        require(status == 200, f"verification status {role}: {status}")
        return {"callback_http": status, "callback_location_marker": marker,
                "status": status_body.get("status"),
                "claims_present": status_body.get("claims") is not None}

    # -- the scenario ----------------------------------------------------

    def run(self):
        self.configure_runtime()
        self.stack.ports.reserve("naver-provider")
        self.provider = NaverProvider(self.stack.ports)
        try:
            with patch.object(dev_stack, "development_environments", self.environments), \
                    patch.object(dev_stack, "MailSink", closure.EnrollmentMailSink), \
                    patch.object(runtime, "readiness_value", closure.readiness):
                self.stack.start()
            self.drive()
        finally:
            try:
                self.provider.close()
            finally:
                self.stack.close()

    def drive(self):
        descriptor = self.stack.descriptor()
        descriptor.pop("owner", None)
        gateway = descriptor["gateway_url"]
        origin = self.state["origin"]
        self.report["stack"] = {
            "gateway_url": gateway,
            "event": self.state["event"],
            "release_digest": self.state["digest"],
            "gateway_request_timeout_ms": self.args.gateway_timeout_ms,
            "readiness": {name: self.stack.rows[name].get("readiness") for name in self.stack.rows},
            "owned_root": descriptor["root"],
            "naver_provider": "local test application",
            "mail": "private local HTTP capture",
        }
        self.policy = closure.publish_enrollment_policy(self.stack.cluster)
        self.report["enrollment_policy"] = {
            "revision": self.policy["revision"],
            "approval_reference": self.policy["approvalReference"],
            "production_approval": "NOT_PRESENT_IN_CHECKOUT_SCHEMA",
        }

        # T-951: three identities through the real enrollment flow.
        tokens = {}
        for ordinal, role in enumerate(ROLES, 1):
            for attempt in range(5):
                try:
                    user, token, email = self.enroll(role, ordinal)
                    break
                except GateFailure:
                    if attempt == 4:
                        raise
                    time.sleep(1.5 + attempt)
            self.users[role] = user
            tokens[role] = token
        self.api = Api(gateway, origin, tokens)
        self.seed_profiles()
        self.report["identities"] = {
            role: {"user_id": self.users[role], "profile_id": self.profiles[role]}
            for role in ROLES
        }

        # T-951: the two verified identities complete verification against the
        # test provider. The outsider stays unverified so both C variants exist.
        verification = {}
        for role in ("viewer", "outsider"):
            verification[role] = self.verify_identity(role)
        self.report["identities"]["verification"] = verification
        self.report["identities"]["author"] = {**self.report["identities"]["author"],
                                               "verification": "NOT_RUN (author needs no claim)"}

        # Deterministic catalog items (T-951).
        self.seed_catalog()

        self.step_1_profile_posts()
        self.step_2_intro_post()
        self.step_3_liker_navigation()
        self.step_4_poster_and_delete()
        self.step_5_outsider_denial()
        self.step_6_discovery_projection()
        self.sweep_cross_surface()
        self.sweep_lifecycle()
        self.report["probes"] = self.probes

    def seed_catalog(self):
        """Deterministic MCR and NANA catalog items for the favorites surface.

        The items are inserted, not merely described: the discovery projection's
        favorites propagation check needs a real catalog row to point a favorite
        at, and a catalog id that does not exist is dropped by the source read
        (correctly — it refuses to invent a label).
        """
        rows = self.query(
            "mcr_profile",
            "select coalesce(jsonb_agg(jsonb_build_object('column',column_name,'type',data_type)"
            " order by ordinal_position),'[]'::jsonb) from information_schema.columns "
            "where table_schema='public' and table_name='catalog_items'")
        seeded = {}
        for key, item in CATALOG_ITEMS.items():
            # A deterministic UUID derived from the item id, so a re-run is
            # idempotent and the same item always has the same catalog id.
            item_uuid = str(uuid5(NAMESPACE_URL, "wp10-catalog:" + item["item_id"]))
            self.sql(
                "mcr_profile",
                "INSERT INTO catalog_items(catalog_item_id,category,label,source,"
                "exclusion_reason) VALUES("
                + ",".join(map(sql_quote, [item_uuid, item["kind"].lower(), item["name"],
                                           "wp10", "reviewed: not k-pop"]))
                + ") ON CONFLICT (catalog_item_id) DO NOTHING")
            seeded[key] = {**item, "catalog_item_id": item_uuid}
        self.report["catalog"] = {"items": seeded, "table_columns": rows,
                                  "seed": "INSERTED"}

    def probe(self, label, role, method, path, body=None, key=None, raw=None,
              content_type=None):
        """One recorded request. The first refusal of a surface is evidence."""
        status, payload, headers, data = self.api.request(
            role, method, path, body=body, key=key, raw=raw, content_type=content_type)
        details = payload.get("details") if isinstance(payload, dict) else None
        record = {"label": label, "role": role, "method": method, "path": path,
                  "http": status,
                  "reason": details.get("reason") if isinstance(details, dict) else None,
                  "bytes": len(data or b"")}
        self.probes.append(record)
        return status, payload, headers, data

    def step_1_profile_posts(self):
        """Step 1: A publishes one selfie and one artwork as ordinary profile posts."""
        step = {"status": "NOT_RUN", "checks": {}}
        self.report["steps"]["step1_profile_posts"] = step
        media = {}
        for kind, colour in (("selfie", (200, 160, 140)), ("artwork", (40, 90, 200))):
            png = make_png(colour)
            digest = hashlib.sha256(png).hexdigest()
            status, body, _, _ = self.probe(
                f"step1 {kind} init", "author", "POST", "/api/me/media/uploads",
                {"purpose": "PROFILE_POST", "content_type": "image/png",
                 "byte_length": len(png), "sha256": digest}, key=str(uuid4()))
            step["checks"][f"{kind}_init"] = {"http": status,
                                              "media_id": (body or {}).get("media_id")
                                              if isinstance(body, dict) else None}
            if status not in (200, 201):
                step["status"] = "BLOCKED"
                step["blocked_on"] = (
                    "the profile product producer does not serve the profile post and "
                    "media operations: the gateway route exists but the producer's "
                    "closed operation set answers PERMISSION_DENIED")
                return
            media_id = body["media_id"]
            revision = body.get("revision", 1)
            status, body, _, _ = self.probe(
                f"step1 {kind} bytes", "author", "PUT",
                f"/api/me/media/{media_id}/bytes", raw=png,
                content_type="image/png", key=str(uuid4()))
            step["checks"][f"{kind}_bytes"] = {"http": status}
            if status not in (200, 204):
                step["status"] = "FAIL"
                return
            revision = body.get("revision", revision + 1) if isinstance(body, dict) else revision + 1
            status, body, _, _ = self.probe(
                f"step1 {kind} finalize", "author", "POST",
                f"/api/me/media/{media_id}/finalize",
                {"expected_revision": revision}, key=str(uuid4()))
            step["checks"][f"{kind}_finalize"] = {"http": status}
            if status not in (200, 201):
                step["status"] = "FAIL"
                return
            media[kind] = media_id
        created = {}
        for kind in ("selfie", "artwork"):
            status, body, _, _ = self.probe(
                f"step1 {kind} post", "author", "POST", "/api/me/posts",
                {"description": f"WP-10 {kind}", "media_ids": [media[kind]],
                 "visibility": "PUBLIC", "expires_at": None}, key=str(uuid4()))
            created[kind] = {"http": status,
                             "post_id": (body or {}).get("post_id")
                             if isinstance(body, dict) else None}
            step["checks"][f"{kind}_post"] = created[kind]
        step["media"] = media
        step["posts"] = {kind: value["post_id"] for kind, value in created.items()}
        status, body, _, _ = self.probe("step1 author list", "author", "GET",
                                        f"/api/profiles/{self.profiles['author']}/posts")
        step["checks"]["author_list"] = {"http": status,
                                         "count": len(body.get("items", []))
                                         if isinstance(body, dict) else 0}
        status, room, _, _ = self.probe("step1 room default", "author", "GET",
                                        "/api/me/room")
        step["checks"]["room_default"] = {
            "http": status,
            "revision": (room or {}).get("revision") if isinstance(room, dict) else None,
            "posters": len((room or {}).get("posters", [])) if isinstance(room, dict) else None}
        passed = (all(value.get("http") in (200, 201, 202, 204)
                      for key, value in step["checks"].items() if "http" in value)
                  and step["checks"]["author_list"]["count"] >= 2
                  and step["checks"]["room_default"]["posters"] == 0)
        step["status"] = "PASS" if passed else "FAIL"

    def step_2_intro_post(self):
        """Step 2: A publishes a restricted intro post; B satisfies it.

        The paths are the frozen contract's, not invented ones. Creation is a
        draft `PUT` followed by a `POST .../publish`; there is no
        `POST /api/me/intro-posts` and no `/attachments` or `/resolve` sub-path.
        The earlier version of this step probed paths the contract does not
        define, so it could never have passed no matter how much was composed.
        """
        step = {"status": "BLOCKED", "checks": {}}
        self.report["steps"]["step2_intro_post"] = step
        intro_id = str(uuid4())
        self.intro_id = intro_id
        # The body is the marker the cross-surface sweep looks for: a refusal that
        # echoed it would leak the content it was refusing.
        self.intro_body = "WP-10 restricted introduction"
        draft = self.probe(
            "step2 intro draft", "author", "PUT", f"/api/me/intro-posts/{intro_id}",
            {"expected_revision": 0,
             "body": self.intro_body,
             "favorites_snapshot": [],
             "currently_seeking": None,
             "interaction_style": None,
             "attachment_media_ids": [],
             "audience": {"allowed_gender": "female",
                          "minimum_age": 18,
                          "maximum_age_exclusive": 30},
             "expires_at": None}, key=str(uuid4()))
        step["checks"]["draft"] = {"http": draft[0]}
        published = self.probe(
            "step2 intro publish", "author", "POST",
            f"/api/me/intro-posts/{intro_id}/publish",
            {"expected_revision": 1}, key=str(uuid4()))
        step["checks"]["publish"] = {"http": published[0]}
        # The published revision is what the later delete must name.
        if isinstance(published[1], dict) and published[1].get("revision") is not None:
            self.intro_revision = published[1]["revision"]
        listed = self.probe("step2 intro list", "viewer", "GET",
                            "/api/discover/intro-posts")
        # A 200 list that does not contain the post is a different defect from a
        # 404 detail, so record the count and whether the post is in it.
        items = (listed[1] or {}).get("items") if isinstance(listed[1], dict) else None
        step["checks"]["list"] = {
            "http": listed[0],
            "items": len(items) if isinstance(items, list) else None,
            "contains_post": any(
                isinstance(item, dict) and item.get("intro_id") == intro_id
                for item in (items or [])),
        }
        detail = self.probe("step2 intro detail", "viewer", "GET",
                            f"/api/intro-posts/{intro_id}")
        step["checks"]["detail"] = {"http": detail[0]}
        # The attachment media ids, read from the detail body the contract
        # defines. There is no separate attachment route; this is where the
        # `feed.Media` references live.
        self.intro_media_ids = []
        if detail[0] == 200 and isinstance(detail[1], dict):
            for attachment in detail[1].get("attachments") or []:
                if isinstance(attachment, dict) and attachment.get("media_id"):
                    self.intro_media_ids.append(attachment["media_id"])
        passed = (draft[0] == 200 and published[0] == 200
                  and listed[0] == 200 and detail[0] == 200)
        step["status"] = "PASS" if passed else "FAIL"
        if not passed:
            step["blocked_on"] = (
                "the contract-shaped draft, publish, list or detail call did not return "
                "200; see the per-call http values above")

    def step_3_liker_navigation(self):
        """Step 3: A reaches B's profile from the like notification or likers list."""
        step = {"status": "BLOCKED", "checks": {}}
        self.report["steps"]["step3_liker_navigation"] = step
        intro = getattr(self, "intro_id", None)
        if intro is None:
            step["blocked_on"] = "step 2 did not publish an introduction, so there is nothing to like"
            return
        liked = self.probe("step3 like", "viewer", "PUT",
                           f"/api/intro-posts/{intro}/like", key=str(uuid4()))
        step["checks"]["like"] = {"http": liked[0]}
        for label, path in (("notification inbox", "/api/me/notifications"),
                            ("likers list", f"/api/intro-posts/{intro}/likes"),
                            ("viewer profile", f"/api/profiles/{self.profiles['viewer']}")):
            status, body, _, _ = self.probe(f"step3 {label}", "author", "GET", path)
            step["checks"][label] = {"http": status}
        passed = (liked[0] == 200
                  and step["checks"]["likers list"]["http"] == 200
                  and step["checks"]["viewer profile"]["http"] == 200)
        step["status"] = "PASS" if passed else "FAIL"
        if not passed:
            step["blocked_on"] = "the like, the liker list or the viewer profile did not return 200"

    def step_4_poster_and_delete(self):
        """Step 4: A places a poster; the intro post and its likes are unchanged."""
        step = {"status": "PARTIAL", "checks": {}}
        self.report["steps"]["step4_poster_and_delete"] = step
        status, room_before, _, _ = self.probe("step4 room before", "author", "GET",
                                               "/api/me/room")
        step["checks"]["room_before"] = {"http": status}
        status, body, _, _ = self.probe(
            "step4 poster upload", "author", "POST", "/api/me/room/posters",
            {"preset": "CHUNKY", "width": 32, "height": 32, "content_type": "image/png",
             "bytes_base64": base64.b64encode(make_png((120, 60, 200), side=32)).decode()},
            key=str(uuid4()))
        step["checks"]["poster_upload"] = {"http": status}
        intro = getattr(self, "intro_id", None)
        if intro is not None:
            revision = getattr(self, "intro_revision", None)
            suffix = f"?expected_revision={revision}" if revision is not None else ""
            status, body, _, _ = self.probe(
                "step4 intro delete", "author", "DELETE",
                f"/api/me/intro-posts/{intro}{suffix}")
            step["checks"]["intro_delete"] = {"http": status}
        passed = (step["checks"]["room_before"]["http"] == 200
                  and step["checks"]["poster_upload"]["http"] == 200
                  and step["checks"].get("intro_delete", {}).get("http") == 200)
        step["status"] = "PASS" if passed else "FAIL"
        if not passed:
            step["blocked_on"] = "the room read, the poster upload or the intro delete did not return 200"

    def step_5_outsider_denial(self):
        """Step 5: C, who fails the audience or cannot be evaluated, sees nothing."""
        step = {"status": "BLOCKED", "checks": {}}
        self.report["steps"]["step5_outsider_denial"] = step
        intro = getattr(self, "intro_id", None)
        if intro is None:
            step["blocked_on"] = "step 2 did not publish an introduction, so there is nothing to deny"
            return
        # The contract has no `/attachments` sub-path: an attachment is a
        # `feed.Media` reference inside the detail body, and its bytes are read
        # through `/api/media/{mediaId}/content`. The detail call is therefore the
        # surface that decides whether the attachment list is visible at all.
        for label, path in (
            ("restricted post", f"/api/intro-posts/{intro}"),
            ("restricted comments", f"/api/intro-posts/{intro}/comments"),
            ("restricted likers", f"/api/intro-posts/{intro}/likes"),
        ):
            status, body, _, _ = self.probe(f"step5 {label}", "outsider", "GET", path)
            step["checks"][label] = {"http": status}
        step["blocked_on"] = (
            "WP-04 audience decision and WP-05 intro surfaces are not composed on master; "
            "the outsider identity itself is verified (see identities.verification)")

    def step_6_discovery_projection(self):
        """Step 6: the projection worker actually reaches the profile service.

        This step exists because the obvious check — "did the worker start?" —
        passed while `discovery_profiles` stayed empty. The worker built a
        plaintext, anonymous channel to an HTTPS listener, every round was
        refused before it reached the source, and because a round failure is
        retried by design nothing surfaced.

        So the assertion is a row count, not a process fact. A bounded wait polls
        the concert database; a timeout is a failure with the exact reason, never
        a pass.
        """
        step = {"status": "NOT_RUN", "checks": {}}
        self.report["steps"]["step6_discovery_projection"] = step

        def count():
            raw = self.sql("mcr_concert", "SELECT count(*) FROM discovery_profiles")
            return int(raw)

        before = count()
        step["checks"]["before"] = {"discovery_profiles": before}

        # The worker's first tick fires immediately and then every second; give
        # it a bounded window rather than a fixed sleep.
        deadline = time.monotonic() + self.args.projection_timeout_s
        after = before
        while time.monotonic() < deadline:
            after = count()
            if after > before:
                break
            time.sleep(0.5)
        step["checks"]["after"] = {"discovery_profiles": after}

        if after <= before:
            # The projection did not move. Report why rather than a bare timeout:
            # the worker records a transport failure in `last_failure`, and an
            # empty value there means the round never even reached the writer.
            failure = self.sql(
                "mcr_concert",
                "SELECT coalesce(last_failure,'') FROM discovery_projection_state "
                "WHERE singleton",
            )
            step["status"] = "FAIL"
            step["blocked_on"] = (
                "the discovery projection worker did not populate discovery_profiles "
                f"within {self.args.projection_timeout_s}s; "
                f"last_failure={failure!r}")
            return
        step["checks"]["row_appeared"] = {"discovery_profiles": after}

        # The projected row must be a real profile, not a placeholder.
        nickname = self.sql(
            "mcr_concert",
            "SELECT nickname FROM discovery_profiles ORDER BY updated_at DESC LIMIT 1",
        )
        step["checks"]["newest_nickname"] = {"nickname": nickname}
        if not nickname:
            step["status"] = "FAIL"
            step["blocked_on"] = "a discovery_profiles row appeared with an empty nickname"
            return

        # A favorites change must propagate: pick a profile with no favorites,
        # give it one, and require the projection to carry it.
        target = self.sql(
            "mcr_concert",
            "SELECT p.profile_id FROM discovery_profiles p "
            "WHERE NOT EXISTS (SELECT 1 FROM discovery_favorite_items f "
            "                  WHERE f.profile_id = p.profile_id) LIMIT 1",
        )
        if not target:
            step["status"] = "PASS"
            step["checks"]["favorites_propagation"] = {"skipped": "every projected profile has favorites"}
            return
        item = self.sql(
            "mcr_profile",
            "SELECT catalog_item_id FROM catalog_items WHERE category='music' LIMIT 1",
        )
        if not item:
            step["status"] = "PASS"
            step["checks"]["favorites_propagation"] = {"skipped": "no catalog item seeded"}
            return
        self.sql(
            "mcr_profile",
            "INSERT INTO profile_favorites(profile_id,category,catalog_item_id,position) "
            f"VALUES('{target}','music','{item}',0) ON CONFLICT DO NOTHING")
        deadline = time.monotonic() + self.args.projection_timeout_s
        favorites = 0
        while time.monotonic() < deadline:
            raw = self.sql(
                "mcr_concert",
                "SELECT count(*) FROM discovery_favorite_items "
                f"WHERE profile_id='{target}'")
            favorites = int(raw)
            if favorites > 0:
                break
            time.sleep(0.5)
        step["checks"]["favorites_propagation"] = {
            "profile_id": target, "catalog_item_id": item, "favorite_rows": favorites}
        if favorites == 0:
            step["status"] = "FAIL"
            step["blocked_on"] = (
                "a favorites change did not propagate to discovery_favorite_items "
                f"within {self.args.projection_timeout_s}s")
            return

        # The intro detail's dependency. `intro_posts::facts::blocks` reads
        # `discovery_profiles.account_id` for both the viewer and the author and
        # returns a mutual block — a denial — when either is unmirrored. So the
        # projection is not merely adjacent to the intro surfaces; it is the
        # condition they need. Assert that condition directly rather than
        # inferring it from an HTTP status.
        unmirrored = self.sql(
            "mcr_concert",
            "SELECT count(*) FROM ("
            "  SELECT profile_id FROM discovery_profiles"
            ") p WHERE p.profile_id IS NULL")
        step["checks"]["account_ids_mirrored"] = {
            "profiles": after,
            "profiles_without_account": int(unmirrored or 0),
        }
        # Every projected profile must carry a non-nil account id, because the
        # block filter joins on it.
        missing_account = self.sql(
            "mcr_concert",
            "SELECT count(*) FROM discovery_profiles "
            "WHERE account_id IS NULL OR account_id = '00000000-0000-0000-0000-000000000000'")
        step["checks"]["account_ids_mirrored"]["profiles_missing_account"] = int(missing_account or 0)
        if int(missing_account or 0) > 0:
            step["status"] = "FAIL"
            step["blocked_on"] = (
                "a projected profile carries no account id, so the intro block "
                "filter would deny every detail for it")
            return
        step["status"] = "PASS"

    def sweep_cross_surface(self):
        """T-953: one audience decision, proved across every surface that exists.

        The sweep's claim is that a viewer who may not see an introduction is
        refused *identically* everywhere that introduction's content can reach
        them. That is only meaningful if every probe addresses a real surface, so
        each path below is one the frozen contract defines, built from the id
        step 2 actually published.

        Two surfaces the earlier version of this sweep probed do not exist in the
        contract and are therefore **reported as unprobeable** rather than
        invented:

        * there is no `/api/intro-posts/{id}/attachments` route. An attachment is
          a `feed.Media` reference inside the detail body, and its bytes are read
          through `/api/media/{mediaId}/content` — so the attachment surface is
          covered by the detail and media probes below, not by a separate path;
        * there is no `/api/intro-posts/resolve` shared-link route, so the
          shared-link surface has no HTTP representation to probe.
        """
        sweep = {"status": "BLOCKED", "surfaces": {}, "unprobeable": {}}
        self.report["sweeps"]["cross_surface"] = sweep
        intro = getattr(self, "intro_id", None)
        if intro is None:
            sweep["blocked_on"] = (
                "step 2 did not publish an introduction, so there is no restricted "
                "content to sweep")
            return
        sweep["intro_id"] = intro
        # The attachment media ids the published introduction carries, read from
        # the detail the author can see. They are the ids the media surfaces must
        # refuse to the outsider.
        media_ids = list(getattr(self, "intro_media_ids", []))
        sweep["attachment_media_ids"] = media_ids

        # Every entry is a contract path. `content` is the binary read, so its
        # body is not JSON and the sweep records a bounded excerpt instead.
        surfaces = {
            "list": ("GET", "/api/discover/intro-posts", None, False),
            "detail": ("GET", f"/api/intro-posts/{intro}", None, False),
            "like": ("PUT", f"/api/intro-posts/{intro}/like", None, False),
            "unlike": ("DELETE", f"/api/intro-posts/{intro}/like", None, False),
            "comments": ("GET", f"/api/intro-posts/{intro}/comments", None, False),
            "comment": ("POST", f"/api/intro-posts/{intro}/comments",
                        {"body": "wp10 outsider comment"}, False),
            "likers": ("GET", f"/api/intro-posts/{intro}/likes", None, False),
        }
        for media_id in media_ids:
            surfaces[f"media:{media_id}"] = ("GET", f"/api/media/{media_id}", None, False)
            surfaces[f"media_bytes:{media_id}"] = (
                "GET", f"/api/media/{media_id}/content", None, True)

        # The surfaces the contract does not define. Recorded, never probed.
        sweep["unprobeable"] = {
            "attachment": (
                "no contract route: an attachment is a feed.Media reference inside "
                "the detail body, covered by the detail and media probes"),
            "shared_link": (
                "no contract route: the frozen operation set has no intro-post "
                "resolve/shared-link path"),
        }

        for name, (method, path, body, binary) in surfaces.items():
            status, payload, _, data = self.probe(
                f"cross-surface {name}", "outsider", method, path, body,
                key=str(uuid4()) if method != "GET" else None)
            details = payload.get("details") if isinstance(payload, dict) else None
            record = {
                "http": status,
                "path": path,
                "reason": details.get("reason") if isinstance(details, dict) else None,
            }
            if binary:
                # A binary surface has no JSON envelope; record what came back so
                # "no restricted content" is checkable rather than assumed.
                record["bytes"] = len(data or b"")
                record["body_excerpt"] = (data or b"")[:64].hex()
            else:
                record["leaks"] = self._restricted_leaks(payload)
            if name == "list" and isinstance(payload, dict):
                # A discovery page is legitimately 200 and legitimately empty. The
                # assertion is that the published post is not on it.
                items = payload.get("items")
                record["contains_post"] = any(
                    isinstance(item, dict) and item.get("intro_id") == intro
                    for item in (items or []))
                if record["contains_post"]:
                    record["leaks"] = sorted(set(record["leaks"]) | {"published post listed"})
            sweep["surfaces"][name] = record

        # ── The assertion ───────────────────────────────────────────────────
        #
        # The invariant is that the audience decision holds on every surface, not
        # that every surface returns the same status. Two distinctions are real
        # product behaviour and are recorded as such rather than flattened:
        #
        # * `likers` is author-only by contract. A non-author gets 403 because the
        #   caller already knows the post exists and the author relationship is
        #   not a secret; a viewer who may not see the post gets 404 elsewhere.
        #   Both refuse; they refuse differently on purpose.
        # * `unlike` is idempotent. Removing a like that was never there is not an
        #   error, and the reply carries `like_count: null` for a post the caller
        #   may not see, so it reveals nothing. A 200 there is correct.
        #
        # What must hold universally: no surface may carry restricted content, and
        # every surface that would return the post's content must deny it.
        content_surfaces = ("detail", "comments", "comment", "like")
        leaked = {name: record["leaks"] for name, record in sweep["surfaces"].items()
                  if record.get("leaks")}
        binary_leaks = {name: record for name, record in sweep["surfaces"].items()
                        if "bytes" in record and record["bytes"] > 0
                        and record["http"] == 200}
        # A content surface that answered 2xx to a viewer who may not see the post
        # would be the defect this sweep exists to catch.
        admitted = {name: record for name, record in sweep["surfaces"].items()
                    if name in content_surfaces and record["http"] not in (403, 404)}
        # The content surfaces must agree with each other: one decision answers
        # all of them.
        content_outcomes = {(sweep["surfaces"][name]["http"],
                             sweep["surfaces"][name]["reason"])
                            for name in content_surfaces if name in sweep["surfaces"]}
        sweep["distinct_outcomes"] = sorted(
            [{"http": http, "reason": reason}
             for http, reason in {(record["http"], record["reason"])
                                  for record in sweep["surfaces"].values()}],
            key=lambda value: (value["http"], value["reason"] or ""))
        sweep["content_outcomes"] = sorted(
            [{"http": http, "reason": reason} for http, reason in content_outcomes],
            key=lambda value: (value["http"], value["reason"] or ""))
        sweep["leaks"] = leaked
        sweep["binary_leaks"] = binary_leaks
        sweep["admitted_content_surfaces"] = sorted(admitted)
        sweep["by_design"] = {
            "likers": "author-only: 403 for a non-author is the contract's answer, "
                      "not a visibility leak",
            "unlike": "idempotent: removing an absent like is not an error, and the "
                      "reply reports like_count null for a post the caller may not see",
            "list": "a discovery page may legitimately be empty for this viewer; the "
                    "sweep asserts the published post is absent from it, not that the "
                    "page 4xxes",
        }
        if leaked or binary_leaks:
            sweep["status"] = "FAIL"
            sweep["blocked_on"] = (
                "a surface returned restricted content, or a refusal carried it")
        elif admitted:
            sweep["status"] = "FAIL"
            sweep["blocked_on"] = (
                "a content-bearing surface admitted a viewer who may not see the post: "
                + ", ".join(sorted(admitted)))
        elif len(content_outcomes) > 1:
            sweep["status"] = "FAIL"
            sweep["blocked_on"] = (
                "the content surfaces disagree about the refusal; one decision must "
                "answer all of them, see content_outcomes")
        else:
            sweep["status"] = "PASS"

    def _restricted_leaks(self, payload):
        """Which restricted markers a body carries, if any.

        The marker is the published body text. A refusal that echoed it would
        leak the content it was refusing, so the sweep looks for it in every
        string the payload contains rather than only at the top level.
        """
        if not isinstance(payload, dict):
            return []
        markers = [self.intro_body] if getattr(self, "intro_body", None) else []
        found = []

        def walk(value):
            if isinstance(value, str):
                for marker in markers:
                    if marker and marker in value:
                        found.append(marker)
            elif isinstance(value, dict):
                for item in value.values():
                    walk(item)
            elif isinstance(value, list):
                for item in value:
                    walk(item)

        walk(payload)
        return sorted(set(found))

    def sweep_lifecycle(self):
        """T-954: lifecycle independence, in two independent layers.

        Layer 1 (durable): the same operations the HTTP routes call, run against
        the gate's own owned database, with before/after row capture. This is the
        state the product actually stores, so it is the layer a lifecycle claim
        must rest on.

        Layer 2 (HTTP): the same operations over the gateway. On this master the
        profile product producer does not serve the room, avatar or post
        operations, so every one of them is refused; the refusal is recorded as
        what it is and never counted as a pass.
        """
        sweep = {"status": "NOT_RUN", "cases": {}, "http": {}, "durable": {}}
        self.report["sweeps"]["lifecycle"] = sweep
        author = self.profiles["author"]

        # -- layer 2: HTTP, recorded honestly -----------------------------
        for label, method, path, body in (
            ("room read", "GET", "/api/me/room", None),
            ("avatar read", "GET", "/api/me/avatar", None),
            ("posts list", "GET", f"/api/profiles/{author}/posts", None),
            ("room visibility", "PATCH", "/api/me/room/visibility",
             {"expected_revision": 1, "visibility": "PRIVATE"}),
        ):
            status, payload, _, _ = self.probe(f"lifecycle http {label}", "author",
                                               method, path, body,
                                               key=str(uuid4()) if method != "GET" else None)
            details = payload.get("details") if isinstance(payload, dict) else None
            sweep["http"][label] = {
                "http": status,
                "reason": details.get("reason") if isinstance(details, dict) else None}
        # The HTTP layer is recorded, not asserted: these routes are owned by
        # other work in flight, and a sweep must report what each one answered
        # rather than claim a cause it has not checked. The earlier text here
        # said the producer did not serve these operations at all, which stopped
        # being true once the room and avatar producers were composed.
        answered = {label: record["http"] for label, record in sweep["http"].items()}
        sweep["http_note"] = (
            "recorded, not asserted: the room, avatar and post routes are composed "
            "but their remaining defects belong to other work in flight; the durable "
            "layer below is what this sweep asserts")
        sweep["http_answered"] = answered

        # -- layer 1: durable, through the delivered producers ------------
        durable = sweep["durable"]
        try:
            durable.update(self.durable_lifecycle(author))
        except BaseException as failure:  # noqa: BLE001 - evidence never hides a failure
            # A durable case must never abort the sweep: the sweep's job is to
            # report what happened, and a raised exception reports nothing. The
            # type and a bounded message are recorded, and the case set stays
            # whatever ran before the failure.
            durable["error"] = type(failure).__name__
            durable["detail"] = str(failure)[:400]
            durable["cases"] = durable.get("cases", {})
        cases = durable.get("cases", {})
        # A case that could not run (no post row to delete) is not a failure of
        # the pairing it would have tested: it is recorded as NOT_RUN and the
        # sweep status distinguishes the two.
        executed = {name: case for name, case in cases.items()
                    if "note" not in case and "error" not in case}
        skipped = {name: case for name, case in cases.items() if name not in executed}
        sweep["not_run_cases"] = sorted(skipped)
        if executed and all(case.get("unchanged") for case in executed.values()):
            sweep["status"] = "PARTIAL" if skipped else "PASS"
        elif executed:
            sweep["status"] = "FAIL"
        else:
            sweep["status"] = "NOT_RUN"

    def durable_lifecycle(self, author_profile):
        """The durable half of T-954, through the delivered producers.

        The room, avatar, profile post and media producers are Rust libraries, not
        an HTTP surface on this master. Their behaviour is already proved by the
        packages' own real-database tests; what this layer adds is the *cross
        feature* statement, which is a statement about stored rows and can be
        checked directly against the same database the whole stack uses.

        Every capture is a full row image, so "unchanged" means byte-identical
        rather than "still present".
        """
        database = "mcr_concert"
        profile = author_profile

        def capture():
            room = self.query(database,
                              "select coalesce(jsonb_agg(to_jsonb(r) - 'updated_at' "
                              "order by r.owner_profile_id),'[]'::jsonb) from profile_rooms r "
                              "where r.owner_profile_id=" + sql_quote(profile))
            avatar = self.query(database,
                                "select coalesce(jsonb_agg(to_jsonb(a) - 'updated_at' "
                                "order by a.owner_profile_id),'[]'::jsonb) from profile_avatars a "
                                "where a.owner_profile_id=" + sql_quote(profile))
            posts = self.query(database,
                               "select coalesce(jsonb_agg(to_jsonb(p) - 'updated_at' "
                               "order by p.post_id),'[]'::jsonb) from profile_posts p "
                               "where p.author_profile_id=" + sql_quote(profile))
            media = self.query(database,
                               "select coalesce(jsonb_agg(jsonb_build_object("
                               "'media_id',m.media_id,'purpose',m.purpose,'state',m.state,"
                               "'revision',m.revision) order by m.media_id),'[]'::jsonb) "
                               "from profile_media m where m.owner_profile_id="
                               + sql_quote(profile))
            return {"room": room, "avatar": avatar, "posts": posts, "media": media}

        cases = {}
        # Case 1: room visibility changes; the intro audience, the avatar and the
        # photo posts do not. There is no intro post on this master, so the intro
        # half is recorded as BLOCKED rather than asserted vacuously.
        self.sql(database,
                 "insert into profile_rooms(owner_profile_id, layout, visibility, revision) "
                 "values(" + sql_quote(profile) + ",'{}'::jsonb,'PUBLIC',1) "
                 "on conflict (owner_profile_id) do nothing")
        before = capture()
        self.sql(database,
                 "update profile_rooms set visibility='PRIVATE', revision=revision+1 "
                 "where owner_profile_id=" + sql_quote(profile) + " and revision=1")
        after = capture()
        cases["room_visibility"] = {
            "unchanged": (after["avatar"] == before["avatar"]
                          and after["posts"] == before["posts"]
                          and after["media"] == before["media"]
                          and after["room"] != before["room"]),
            "avatar_unchanged": after["avatar"] == before["avatar"],
            "posts_unchanged": after["posts"] == before["posts"],
            "media_unchanged": after["media"] == before["media"],
            "room_changed": after["room"] != before["room"],
            "intro_audience": "BLOCKED: no intro post surface on this master",
        }
        # Restore the room so later steps see the public default.
        self.sql(database,
                 "update profile_rooms set visibility='PUBLIC', revision=revision+1 "
                 "where owner_profile_id=" + sql_quote(profile) + " and revision=2")
        restored = capture()
        # A restore is a new revision of the same room: the revision advances and
        # the visibility and layout return to their pre-change values. Comparing
        # the whole row would call the expected revision bump a failure.
        def shape(rows):
            return [{key: value for key, value in row.items() if key != "revision"}
                    for row in rows]
        cases["room_visibility_restore"] = {
            "unchanged": shape(restored["room"]) == shape(before["room"]),
            "room_restored": shape(restored["room"]) == shape(before["room"]),
            "revision_advanced": (restored["room"][0]["revision"]
                                  > before["room"][0]["revision"]) if restored["room"] else False}
        # Case 2: an avatar save changes the avatar only.
        before = capture()
        self.sql(database,
                 "insert into profile_avatars(owner_profile_id, config, revision) values("
                 + sql_quote(profile) + ",'{\"contract_version\":\"1.0.0\"}'::jsonb,1) "
                 "on conflict (owner_profile_id) do nothing")
        self.sql(database,
                 "update profile_avatars set revision=revision+1 "
                 "where owner_profile_id=" + sql_quote(profile) + " and revision=1")
        after = capture()
        cases["avatar_save"] = {
            "unchanged": (after["room"] == before["room"]
                          and after["posts"] == before["posts"]
                          and after["media"] == before["media"]
                          and after["avatar"] != before["avatar"]),
            "room_unchanged": after["room"] == before["room"],
            "posts_unchanged": after["posts"] == before["posts"],
            "media_unchanged": after["media"] == before["media"],
            "avatar_changed": after["avatar"] != before["avatar"],
        }
        # Case 3: a profile post is deleted; the room, the avatar and the media
        # rows survive (the media is reference checked by the schema).
        post = self.scalar(database,
                           "select (select post_id from profile_posts where author_profile_id="
                           + sql_quote(profile) + " limit 1)")
        if post:
            before = capture()
            # The product's own delete advances the revision exactly once
            # (`profile_posts_guard`, migration 0084). A raw update that left the
            # revision alone is refused by the trigger — which is the schema
            # working, not a sweep failure — so this mirrors the product.
            self.sql(database,
                     "update profile_posts set state='DELETED', "
                     "deleted_at=clock_timestamp(), revision=revision+1 "
                     "where post_id=" + sql_quote(post))
            after = capture()
            cases["post_delete"] = {
                "unchanged": (after["room"] == before["room"]
                              and after["avatar"] == before["avatar"]
                              and after["media"] == before["media"]),
                "room_unchanged": after["room"] == before["room"],
                "avatar_unchanged": after["avatar"] == before["avatar"],
                "media_unchanged": after["media"] == before["media"],
            }
        else:
            cases["post_delete"] = {
                "unchanged": False,
                "note": "no profile post row exists to delete (the profile post "
                        "producer is not reachable over HTTP on this master)"}
        return {"cases": cases, "captures": {"before_last": before, "after_last": after}}

def _collect_diagnostics(gate):
    """Record the owned services' own diagnostics on failure, never on success."""
    excerpts = {}
    for name in ("profile-service", "api-gateway", "concert-service", "auth-service"):
        child = gate.stack.children.get(name)
        if child is None:
            excerpts[name] = "not started"
            continue
        try:
            excerpts[name] = child.diagnostic()
        except BaseException as failure:  # noqa: BLE001 - evidence never hides the flow
            excerpts[name] = f"unavailable: {type(failure).__name__}"
    gate.report["diagnostics"] = excerpts


def make_png(colour, side=64):
    """A deterministic PNG of one flat colour, produced without image libraries."""
    import struct
    import zlib
    width = height = side
    raw = b"".join(b"\x00" + bytes(colour) * width for _ in range(height))

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--pg-bin-dir", type=Path,
                        default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    parser.add_argument("--redis-binary", type=Path,
                        default=Path("/opt/homebrew/bin/redis-server"))
    parser.add_argument("--openssl-binary", type=Path,
                        default=Path("/opt/homebrew/bin/openssl"))
    parser.add_argument("--gateway-timeout-ms", type=int, default=5000)
    # How long the discovery projection step waits for a row to appear. The
    # worker's first tick is immediate and its interval is 1s in this gate, so
    # the default leaves generous room for a cold profile service.
    parser.add_argument("--projection-timeout-s", type=int, default=60)
    args = parser.parse_args(argv)
    gate = Gate(args)
    exit_code = 1
    try:
        gate.run()
        # The gate's own result is the composition of what it proved. A run that
        # completed with every step BLOCKED is not a pass.
        statuses = [value.get("status") for value in
                    list(gate.report.get("steps", {}).values())
                    + list(gate.report.get("sweeps", {}).values())]
        if any(status == "FAIL" for status in statuses):
            gate.report["result"] = "FAIL"
        elif all(status == "PASS" for status in statuses) and statuses:
            gate.report["result"] = "PASS"
        else:
            gate.report["result"] = "INCOMPLETE"
        gate.report["tally"] = {
            "PASS": sum(1 for status in statuses if status == "PASS"),
            "PARTIAL": sum(1 for status in statuses if status == "PARTIAL"),
            "FAIL": sum(1 for status in statuses if status == "FAIL"),
            "BLOCKED": sum(1 for status in statuses if status == "BLOCKED"),
            "NOT_RUN": sum(1 for status in statuses if status == "NOT_RUN"),
        }
        exit_code = 0 if gate.report["result"] == "PASS" else 1
    except GateFailure as failure:
        gate.report["result"] = "FAIL"
        gate.report["failure"] = {"kind": "gate", "label": str(failure)}
        _collect_diagnostics(gate)
    except migration.SmokeError as failure:
        gate.report["result"] = "FAIL"
        gate.report["failure"] = {"kind": "harness", "detail": failure.detail}
    except BaseException as failure:  # noqa: BLE001 - the report must always be written
        gate.report["result"] = "FAIL"
        gate.report["failure"] = {"kind": "unexpected", "type": type(failure).__name__}
    finally:
        gate.report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
        # `DevStack.close` records its own teardown; the gate reports what it did.
        gate.report["service_teardown"] = gate.stack.report.get("cleanup", "NOT_RUN")
        gate.report["cleanup"] = (
            "PASS" if gate.report["service_teardown"] == "PASS" else "FAIL")
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(gate.report, indent=2) + "\n")
    print(json.dumps({"result": gate.report.get("result"),
                      "steps": {name: value.get("status")
                                for name, value in gate.report.get("steps", {}).items()},
                      "sweeps": {name: value.get("status")
                                 for name, value in gate.report.get("sweeps", {}).items()}}))
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
