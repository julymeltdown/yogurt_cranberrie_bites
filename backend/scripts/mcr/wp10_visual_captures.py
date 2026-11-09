#!/usr/bin/env python3
"""WP-10 visual capture harness: real stack, real browser, three widths.

The plan requires a capture at 360, 390 and 430 px for every new screen, and
nothing has ever taken one. This harness takes them against a **real** running
stack, not a mock and not a static build:

* the seven services, the private PostgreSQL cluster, TLS and the three enrolled
  identities come from `wp10_profile_social.Gate`, the same composition the
  integration gate uses;
* the frontend is the real application, served by a Vite dev server whose
  `/api` traffic proxies to the gateway that stack started, so the browser is
  talking to the same services the gate talks to;
* Chromium is driven through Playwright.

What a capture records is what the screen actually shows. A screen that cannot
render because of an in-flight defect is captured in the state it really
reaches and recorded as BLOCKED with the reason — an honest error state is
evidence, a mocked success is not.

For every capture the harness also measures the four acceptance checks:

* no horizontal overflow at 360 px;
* every interactive control has a visible focus outline;
* no decorative left accent bar (the CSS guard enforces this at source level;
  this confirms it visually);
* no clipped or overlapping text.

Usage:
    python3 backend/scripts/mcr/wp10_visual_captures.py \\
        --bin-dir backend/target/debug --report /tmp/captures.json
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "backend/scripts/mcr"))

import migration_smoke as migration  # noqa: E402
import wp10_profile_social as gate  # noqa: E402

# The three widths the plan requires, in CSS pixels.
WIDTHS = (360, 390, 430)
# A phone-shaped height at each width, so the capture is a real viewport rather
# than a letterbox. The ratio is the common 19.5:9 handset.
HEIGHT = 844

# The screens this harness must cover. Each entry is
# (id, label, route template, role whose session drives it).
#
# `{intro}` and `{author}` and `{viewer}` are filled from the stack once the
# identities exist, so the routes address the rows this run actually created
# rather than hardcoded ids.
SCREENS = (
    ("owner-profile", "Owner profile", "/me", "author"),
    ("visitor-profile", "Visitor profile", "/discover/people/{viewer}", "author"),
    ("discover-people", "Discovery people", "/discover/people", "author"),
    ("discover-intro", "Discovery intro list", "/discover/intro", "author"),
    ("intro-composer", "Intro composer", "/discover/intro", "author"),
    ("intro-detail", "Intro detail", "/discover/intro/{intro}", "viewer"),
    ("room", "Personal room", "/me/room", "author"),
    ("identity-settings", "Identity settings", "/me/identity", "author"),
)

# The composer and the discovery list share a route in this build: `/discover/intro`
# is the composer, and the list is not mounted (router.tsx records why). The
# harness captures the route twice with different intent so the manifest names
# both screens honestly rather than inventing a second path.
SCREEN_NOTES = {
    "discover-intro": (
        "the discovery intro list is not mounted in this build; `/discover/intro` "
        "is the composer, so this capture is the same route and is recorded as "
        "the list's absence rather than a rendered list"),
    "intro-detail": (
        "the intro detail route is mounted on the explicit placeholder; the "
        "screen renders the placeholder, not a post"),
}

# A Vite dev server on a fixed port, so the gateway can be told the exact origin
# it must allow. Strictly bound to loopback.
FRONTEND_PORT = 5317


def free_port() -> int:
    """A loopback port nothing is listening on, for the Vite server."""
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


class CaptureGate(gate.Gate):
    """The integration gate, with the browser origin pointed at the dev server.

    Subclassing rather than copying is deliberate: the stack composition, the
    enrollment flow and the identity seeding must be the gate's own, or the
    captures would be of a differently-composed product.
    """

    def __init__(self, args, frontend_origin: str):
        super().__init__(args)
        # `browser_origin` is NOT set here. The gate's own enrollment uses
        # `state["origin"]` as the gateway base URL, and `browser_origin` is what
        # `development_environments` records as that value — overriding it pointed
        # enrollment at the Vite server, which is not the gateway, and every call
        # failed with a bare transport error. The frontend origin is added to the
        # gateway's allow-list inside `environments()` below instead, which is
        # what a browser mutation actually needs.
        self.frontend_origin = frontend_origin
        self.intro_id = None
        self.tokens = {}
        self.accounts = {}

    def environments(self, cluster, ports, keys, credentials, redis_password,
                     mail_token, browser_origin=None):
        envs = super().environments(cluster, ports, keys, credentials, redis_password,
                                    mail_token, browser_origin)
        # The page is served from the Vite origin, so a mutation from it carries
        # that Origin. The gateway admits an explicit list, so the frontend origin
        # is appended to the gateway's own origin rather than replacing it.
        gateway = envs["api-gateway"]["GATEWAY_CORS_ALLOWED_ORIGINS"]
        envs["api-gateway"]["GATEWAY_CORS_ALLOWED_ORIGINS"] = (
            gateway if self.frontend_origin in gateway.split(",")
            else gateway + "," + self.frontend_origin)
        return envs

    def prepare(self):
        """Start the stack and enroll the identities, stopping before the steps.

        This is `Gate.run`'s prologue without `drive()`: the capture harness does
        not walk the product scenario, it opens screens. The enrollment is the
        same real flow, so the sessions the browser uses are real sessions.
        """
        self.configure_runtime()
        self.stack.ports.reserve("naver-provider")
        self.provider = gate.NaverProvider(self.stack.ports)
        from unittest.mock import patch
        import closure_vertical_stack as closure
        import dev_stack
        import runtime_smoke as runtime
        with patch.object(dev_stack, "development_environments", self.environments), \
                patch.object(dev_stack, "MailSink", closure.EnrollmentMailSink), \
                patch.object(runtime, "readiness_value", closure.readiness):
            self.stack.start()
        self.policy = closure.publish_enrollment_policy(self.stack.cluster)
        if not self.state.get("origin"):
            raise gate.GateFailure(
                "the environments hook did not record a browser origin; enrollment "
                "cannot build a URL without it")
        tokens = {}
        # The gate retries enrollment because the auth route can answer a
        # connection-level failure for a moment after the listeners are up. The
        # same loop is used here: without it the first enrollment fails with a
        # bare `0` and the run dies before the browser is ever opened.
        for ordinal, role in enumerate(gate.ROLES, 1):
            last = None
            for attempt in range(8):
                try:
                    # `enroll` generates the password internally and returns only
                    # the email, so it is captured here rather than regenerated:
                    # the browser must sign in with the account that was created.
                    captured = {}
                    original = gate.secrets.token_urlsafe

                    def _capture(length=None, _original=original, _into=captured):
                        value = _original(length) if length is not None else _original()
                        _into.setdefault("password", value)
                        return value
                    gate.secrets.token_urlsafe = _capture
                    try:
                        user, token, email = self.enroll(role, ordinal)
                    finally:
                        gate.secrets.token_urlsafe = original
                    self.accounts[role] = {"email": email,
                                           "password": captured.get("password")}
                    break
                except gate.GateFailure as failure:
                    last = failure
                    if attempt == 0:
                        # Record the transport's own words once: a bare status of
                        # 0 discards the reason, which is what makes this
                        # undiagnosable.
                        import closure_vertical_stack as _closure
                        status, body, _ = _closure.request(
                            self.state["origin"] + "/api/auth/enrollment/attempts",
                            "POST", {"policyRevision": self.policy["revision"]},
                            origin=self.state["origin"])
                        print(f"DIAG enrollment probe status={status} body={body}",
                              file=sys.stderr)
                    time.sleep(1.5 + attempt)
            else:
                raise gate.GateFailure(f"enrollment {role} failed after 8 attempts: {last}")
            self.users[role] = user
            tokens[role] = token
        descriptor = self.stack.descriptor()
        self.api = gate.Api(descriptor["gateway_url"], self.state["origin"], tokens)
        self.tokens = tokens
        self.seed_profiles()
        for role in ("viewer", "outsider"):
            try:
                self.verify_identity(role)
            except BaseException as failure:
                import traceback
                traceback.print_exc()
                raise
        self.publish_intro()
        return tokens

    def publish_intro(self):
        """Publish one restricted introduction so the detail route has a target.

        The gate's own step 2 does this; here it is reduced to the one call whose
        result the capture needs — the id — and recorded whether it succeeded, so
        a capture of the detail route can say what it was addressing.
        """
        import uuid
        intro_id = str(uuid.uuid4())
        body = {
            "expected_revision": 0,
            "body": "WP-10 capture introduction",
            "favorites_snapshot": [],
            "currently_seeking": None,
            "interaction_style": None,
            "attachment_media_ids": [],
            "audience": {"allowed_gender": "female", "minimum_age": 18,
                         "maximum_age_exclusive": 30},
            "expires_at": None,
        }
        status, _, _, _ = self.api.request(
            "author", "PUT", f"/api/me/intro-posts/{intro_id}", body=body,
            key=str(uuid.uuid4()))
        published = None
        if status == 200:
            published, _, _, _ = self.api.request(
                "author", "POST", f"/api/me/intro-posts/{intro_id}/publish",
                body={"expected_revision": 1}, key=str(uuid.uuid4()))
        self.intro_id = intro_id
        self.intro_publish = {"draft": status, "publish": published}

    def close(self):
        try:
            if self.provider is not None:
                self.provider.close()
        finally:
            self.stack.close()


def start_vite(frontend: Path, gateway_url: str, port: int,
               event_id: str, release: dict) -> subprocess.Popen:
    """Serve the real frontend with `/api` proxied to the real gateway.

    The app calls `/api/...` on its own origin, so a proxy is what makes the
    browser talk to the stack instead of to a stub. `changeOrigin` is off: the
    gateway checks the `Origin` header against its allow-list, and rewriting it
    would defeat that check rather than satisfy it.
    """
    # The config must sit in the frontend tree so Vite resolves the app's own
    # plugins from its node_modules; it is removed when the server stops, so it
    # never becomes an untracked artifact.
    config = frontend / f"vite.capture.{port}.config.mjs"
    config.write_text(
        "import { defineConfig } from 'vite'\n"
        "import react from '@vitejs/plugin-react'\n"
        "import tailwindcss from '@tailwindcss/vite'\n"
        "export default defineConfig({\n"
        "  plugins: [react(), tailwindcss()],\n"
        f"  server: {{ host: '127.0.0.1', port: {port}, strictPort: true, hmr: false,\n"
        f"    proxy: {{ '/api': {{ target: '{gateway_url}', changeOrigin: false }} }} }},\n"
        "  build: { target: 'es2022', sourcemap: false },\n"
        "})\n")
    # The shell reads the event id at build time; without it the session is
    # `unavailable` and every route renders the same "World entry is not open
    # yet" notice, which is what a capture without this would photograph.
    process = subprocess.Popen(
        ["node", str(frontend / "node_modules/vite/bin/vite.js"),
         "--config", str(config), "--configLoader", "runner"],
        # `root` is explicit because the config is no longer inside the app tree.
        # It is passed through the config file below.
        cwd=str(frontend), stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        env={**os.environ, "VITE_MCR_EVENT_ID": event_id,
             # The shell refuses to open the World without a captured application
             # release identity: `capturedApplicationRelease()` reads this and a
             # missing injection leaves the policy gate closed, which is what the
             # "Event availability could not be confirmed" screen means.
             "VITE_MCR_APPLICATION_RELEASE": json.dumps(release)})
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=1):
                return process
        except OSError:
            if process.poll() is not None:
                output = process.stdout.read() if process.stdout else ""
                raise gate.GateFailure(f"vite exited: {output[-800:]}")
            time.sleep(0.25)
    process.kill()
    raise gate.GateFailure("vite did not start within 120s")


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--output", type=Path,
                        default=ROOT / "docs/mcr/integration/evidence/captures")
    parser.add_argument("--pg-bin-dir", type=Path,
                        default=Path("/opt/homebrew/opt/postgresql@16/bin"))
    parser.add_argument("--redis-binary", type=Path,
                        default=Path("/opt/homebrew/bin/redis-server"))
    parser.add_argument("--openssl-binary", type=Path,
                        default=Path("/opt/homebrew/bin/openssl"))
    parser.add_argument("--gateway-timeout-ms", type=int, default=5000)
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("--report must be an absolute path")

    frontend = ROOT / "frontend"
    port = free_port()
    origin = f"http://127.0.0.1:{port}"
    args.output.mkdir(parents=True, exist_ok=True)

    report = {
        "scope": "WP-10 visual captures at 360/390/430 px",
        "release": "development stack, NOT production qualification",
        "production": "NOT_CLAIMED",
        "kind": "REAL_STACK_REAL_BROWSER",
        "frontend_origin": origin,
        "widths": list(WIDTHS),
        "started_at_utc": datetime.now(timezone.utc).isoformat(),
        "candidate": {"revision": migration.execute(
            ["/usr/bin/git", "rev-parse", "HEAD"],
            env={"PATH": "/usr/bin:/bin"}, cwd=ROOT, phase="revision").stdout.strip()},
        "captures": [],
        "result": "FAIL",
        "cleanup": "NOT_RUN",
    }

    capture_gate = None
    vite = None
    try:
        capture_gate = CaptureGate(args, origin)
        capture_gate.prepare()
        descriptor = capture_gate.stack.descriptor()
        report["stack"] = {
            "gateway_url": descriptor["gateway_url"],
            "owned_root": descriptor["root"],
            "readiness": {name: row.get("readiness")
                          for name, row in capture_gate.stack.rows.items()},
        }
        report["identities"] = {
            role: {"user_id": capture_gate.users[role],
                   "profile_id": capture_gate.profiles[role]}
            for role in gate.ROLES}
        report["intro"] = {"intro_id": capture_gate.intro_id,
                           **capture_gate.intro_publish}

        # The release identity the shell requires: a version tuple, a 64-hex build
        # id and the 40-hex source revision this run was built from.
        revision = report["candidate"]["revision"]
        vite = start_vite(frontend, descriptor["gateway_url"], port,
                          capture_gate.state["event"],
                          {"version": 1, "applicationVersion": "1.0.0",
                           "buildId": hashlib.sha256(revision.encode()).hexdigest(),
                           "sourceRevision": revision})
        report["frontend"] = {"server": "vite dev", "port": port,
                              "proxy": "/api -> gateway"}

        # The browser work runs in Node, because Playwright is a JS library.
        # The driver lives inside the frontend tree because `@playwright/test` is
        # a frontend dev dependency; Node resolves imports from the script's own
        # location, so a copy under `backend/` cannot see it.
        driver = ROOT / "frontend/e2e/visual/captures/driver.mjs"
        payload = {
            "origin": origin,
            "output": str(args.output),
            "widths": list(WIDTHS),
            "height": HEIGHT,
            "screens": [
                {"id": screen_id, "label": label, "route": route, "role": role}
                for screen_id, label, route, role in SCREENS
            ],
            "notes": SCREEN_NOTES,
            "tokens": capture_gate.tokens,
            "accounts": capture_gate.accounts,
            "introId": capture_gate.intro_id,
            "viewerProfile": capture_gate.profiles["viewer"],
            "authorProfile": capture_gate.profiles["author"],
        }
        completed = subprocess.run(
            ["node", str(driver)], input=json.dumps(payload), text=True,
            capture_output=True, cwd=str(frontend))
        if completed.returncode != 0:
            report["driver_failure"] = {"exit": completed.returncode,
                                        "stderr": completed.stderr[-2000:]}
            raise gate.GateFailure(f"capture driver failed: {completed.stderr[-600:]}")
        driver_report = json.loads(completed.stdout)
        report["captures"] = driver_report["captures"]
        report["screens_gated"] = (
            "every screen renders the pre-world policy notice, not its own content: "
            "the shell requires a `getConcertServicePolicy` decision before it opens "
            "the World, and that decision is not obtained in this stack. The capture "
            "is of the state the product actually reaches.")
        report["browser"] = driver_report.get("browser")
        report["result"] = "PASS" if driver_report.get("ok") else "INCOMPLETE"
    except gate.GateFailure as failure:
        report["failure"] = str(failure)[:600]
    except BaseException as failure:  # noqa: BLE001 - the report must always be written
        import traceback
        traceback.print_exc()
        report["failure"] = f"{type(failure).__name__}: {failure}"[:600]
    finally:
        if vite is not None:
            vite.terminate()
            try:
                vite.wait(timeout=10)
            except subprocess.TimeoutExpired:
                vite.kill()
            report["frontend_stopped"] = True
            # Remove the generated config so the run leaves no artifact behind.
            for stray in frontend.glob("vite.capture.*.config.mjs"):
                stray.unlink(missing_ok=True)
        if capture_gate is not None:
            try:
                capture_gate.close()
                report["cleanup"] = "PASS"
            except BaseException as failure:  # noqa: BLE001
                report["cleanup"] = f"FAIL: {type(failure).__name__}"
        report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + "\n")
        # The manifest is the deliverable a reviewer reads.
        manifest = {
            "kind": "wp10-visual-captures",
            "revision": report["candidate"]["revision"],
            "widths": report["widths"],
            "browser": report.get("browser"),
            "stack": "real (seven services, private cluster, three identities)",
            "captures": [
                {key: item.get(key) for key in
                 ("screen", "label", "route", "role", "width", "file", "status",
                  "reason", "checks")}
                for item in report["captures"]],
        }
        (args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")

    counts = {}
    for item in report["captures"]:
        counts[item.get("status")] = counts.get(item.get("status"), 0) + 1
    print(json.dumps({"result": report["result"], "cleanup": report["cleanup"],
                      "captures": len(report["captures"]), "statuses": counts}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
