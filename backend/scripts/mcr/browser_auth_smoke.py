#!/usr/bin/env python3
"""Isolated C03 email-auth qualification using the unchanged owned native stack."""
from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import secrets
import signal
import time
from urllib.parse import parse_qs, urlsplit
from uuid import UUID

import dev_stack as dev


class CheckFailure(Exception):
    """Only a fixed check label, never a credential or response body."""


def require(condition, label):
    if not condition:
        raise CheckFailure(label)


class Browser:
    def __init__(self, origin, report):
        self.origin = dev.local_origin(origin)
        self.target = urlsplit(self.origin)
        self.report = report

    def request(self, label, path, payload=None, *, method="POST", expected=200,
                cookie=None, bearer=None, origin=True, intent="browser", extra=None):
        headers = {"Content-Type": "application/json", "X-Correlation-Id": "be03-" + label}
        if origin is not False:
            headers["Origin"] = self.origin if origin is True else origin
        if intent is not None:
            headers["X-MCR-Intent"] = intent
        if cookie:
            headers["Cookie"] = cookie
        if bearer:
            headers["Authorization"] = "Bearer " + bearer
        headers.update(extra or {})
        connection = http.client.HTTPConnection(self.target.hostname, self.target.port, timeout=10)
        try:
            connection.request(method, path, body=json.dumps(payload or {}), headers=headers)
            response = connection.getresponse()
            raw = response.read(65537)
            observed = dict(response.getheaders())
            # Receipts deliberately omit request/response/cookie/token/email values.
            self.report["statuses"][label] = response.status
            require(response.status == expected, label + ":status")
            require(len(raw) <= 65536, label + ":body-limit")
            require(observed.get("cache-control") == "no-store", label + ":no-store")
            require(observed.get("x-correlation-id") == headers["X-Correlation-Id"], label + ":correlation")
            require(observed.get("referrer-policy") == "no-referrer", label + ":referrer")
            require("location" not in observed, label + ":unexpected-redirect")
            data = json.loads(raw) if raw else None
            if expected >= 400 and data is not None:
                require(data.get("traceId") == observed["x-correlation-id"], label + ":error-correlation")
            return observed, data
        finally:
            connection.close()


def refresh_cookie(headers):
    cookie = headers.get("set-cookie", "")
    require("HttpOnly" in cookie and "SameSite=Lax" in cookie and "Path=/" in cookie,
            "refresh-cookie-attributes")
    # The unchanged dev launcher explicitly opts into plain loopback HTTP only.
    return cookie.split(";", 1)[0]


def auth_snapshot(cluster):
    """Read exact state only from this invocation's marker/inode-owned Auth database."""
    cluster.assert_owned()
    tables = ("users", "user_identities", "email_verifications", "refresh_tokens", "login_audit",
              "password_reset_tokens", "oauth_authorizations", "auth_audit_events", "auth_mail_outbox")
    values = [cluster.sql("mcr_auth", "select coalesce(jsonb_agg(row order by row::text)::text, '[]') from "
                          f"(select to_jsonb(t) as row from {table} t) snapshot;") for table in tables]
    return hashlib.sha256("\n".join(values).encode()).hexdigest()


def exercise(stack, report, descriptor):
    browser = Browser(descriptor["gateway_url"], report)
    inbox = stack.cluster.root / "mail"
    seen = set()
    email = "mcr-be03-" + secrets.token_hex(10) + "@example.test"
    unknown = "mcr-unknown-" + secrets.token_hex(10) + "@example.test"
    password, replacement = "é" * 4, "🙂" * 32  # Exact 8/128 UTF-8 byte boundaries.

    def mail(kind):
        deadline = time.monotonic() + 20
        while time.monotonic() < deadline:
            for path in inbox.glob("*.json"):
                require(not path.is_symlink() and path.stat().st_mode & 0o077 == 0, "private-mail")
                value = json.loads(path.read_bytes())
                if value["delivery_id"] not in seen and value["kind"] == kind and value["payload"]["email"] == email:
                    seen.add(value["delivery_id"])
                    return value["payload"]
            time.sleep(0.1)
        raise CheckFailure("actual-mail-timeout")

    _, capabilities = browser.request("capabilities", "/api/auth/capabilities", method="GET", origin=False, intent=None)
    require(capabilities["contract_version"] == "0.1.0" and
            [(p["provider"], p["enabled"]) for p in capabilities["providers"]] ==
            [("EMAIL", True), ("GOOGLE", False), ("APPLE", False), ("X", False)], "capabilities-policy")
    before, mail_count = auth_snapshot(stack.cluster), stack.mail.count
    for label, origin, intent, reason in [
        ("missing-origin", False, "browser", "ORIGIN_NOT_ALLOWED"),
        ("null-origin", "null", "browser", "ORIGIN_NOT_ALLOWED"),
        ("hostile-origin", "https://hostile.invalid", "browser", "ORIGIN_NOT_ALLOWED"),
        ("origin-suffix", browser.origin + ".hostile.invalid", "browser", "ORIGIN_NOT_ALLOWED"),
        ("missing-intent", True, None, "BROWSER_INTENT_REQUIRED"),
        ("wrong-intent", True, "Browser", "BROWSER_INTENT_REQUIRED"),
    ]:
        headers, body = browser.request(label, "/api/auth/email/signup", {"email":email,"password":password},
                                        expected=403, origin=origin, intent=intent)
        require("set-cookie" not in headers and body["details"] == {"reason":reason}, label + ":denial")
    require(auth_snapshot(stack.cluster) == before and stack.mail.count == mail_count, "denial-zero-database-mail-effect")
    report["native_denial_state_unchanged"] = True

    for label, candidate in [("signup-short-utf8", "é" * 3), ("signup-long-utf8", "é" * 65)]:
        browser.request(label, "/api/auth/email/signup", {"email":email,"password":candidate}, expected=400)
    _, signup = browser.request("signup-8-utf8-bytes", "/api/auth/email/signup", {"email":email,"password":password})
    user = str(UUID(signup["userId"]))
    require(signup["userId"] == user, "canonical-auth-uuid")
    verification = mail("VERIFY_EMAIL")
    wrong_code = "000001" if verification["verification_code"] == "000000" else "000000"
    browser.request("verify-wrong", "/api/auth/email/verify", {"email":email,"code":wrong_code}, expected=400)
    _, verified = browser.request("verify", "/api/auth/email/verify", {"email":email,"code":verification["verification_code"]})
    require(verified["userId"] == user, "verify-canonical-identity")
    browser.request("verify-reused", "/api/auth/email/verify", {"email":email,"code":verification["verification_code"]}, expected=400)
    _, neutral = browser.request("resend-unknown", "/api/auth/email/resend", {"email":unknown}, expected=202)
    require(neutral == {"status":"ACCEPTED"}, "resend-neutral")
    browser.request("login-not-normalized", "/api/auth/login", {"email":email,"password":"e\u0301" * 4}, expected=401)
    browser.request("refresh-missing", "/api/auth/refresh", expected=401)
    browser.request("refresh-malformed", "/api/auth/refresh", cookie="refresh_token=malformed", expected=401)
    browser.request("legacy-unavailable", "/auth/login", {"email":email,"password":password}, expected=404)
    browser.request("availability-unavailable", "/api/auth/email/check", {"email":email}, expected=404)
    for provider in ("google", "apple", "x"):
        browser.request("oauth-" + provider, "/api/auth/oauth/" + provider + "/authorize", method="GET", expected=404)
    browser.request("oauth-apple-callback", "/api/auth/oauth/apple/callback", expected=404)

    def login(label, candidate=password):
        headers, body = browser.request(label, "/api/auth/login", {"email":email,"password":candidate})
        require(body["userId"] == user and body["refreshToken"] == "" and body["accessToken"], label + ":identity-token")
        return refresh_cookie(headers), body["accessToken"]

    def refresh(label, cookie):
        headers, body = browser.request(label, "/api/auth/refresh", cookie=cookie)
        require(body["userId"] == user and body["refreshToken"] == "", label + ":identity-token")
        return refresh_cookie(headers)

    first, access = login("login")
    browser.request("account-status", "/api/auth/account", method="GET", bearer=access, expected=404)
    second = refresh("refresh", first)
    browser.request("refresh-immediate-reuse", "/api/auth/refresh", cookie=first, expected=409)
    third = refresh("refresh-family-survives-race", second)
    before = auth_snapshot(stack.cluster)
    headers, _ = browser.request("denied-refresh-cookie", "/api/auth/refresh", cookie=third, origin=False, expected=403)
    require("set-cookie" not in headers and auth_snapshot(stack.cluster) == before, "denied-refresh-does-not-rotate")
    fourth = refresh("refresh-after-denial", third)
    headers, _ = browser.request("logout", "/api/auth/logout", cookie=fourth, expected=204)
    require("Max-Age=0" in headers.get("set-cookie", ""), "logout-clears-cookie")
    browser.request("refresh-after-logout", "/api/auth/refresh", cookie=fourth, expected=401)
    browser.request("delete-after-logout", "/api/auth/account", method="DELETE", bearer=access, expected=401)

    old, _ = login("login-for-reuse")
    current = refresh("rotate-for-reuse", old)
    # Test clocks are changed in this disposable database, never an existing store.
    stack.cluster.sql("mcr_auth", f"update refresh_tokens set rotated_at=now()-interval '60 seconds' where user_id='{user}' and rotated_at is not null;")
    browser.request("refresh-outside-grace-reuse", "/api/auth/refresh", cookie=old, expected=401)
    browser.request("refresh-family-revoked", "/api/auth/refresh", cookie=current, expected=401)
    expired, _ = login("login-for-expiry")
    stack.cluster.sql("mcr_auth", f"update refresh_tokens set expires_at=now()-interval '60 seconds' where user_id='{user}' and revoked_at is null;")
    browser.request("refresh-database-expired", "/api/auth/refresh", cookie=expired, expected=401)

    before_reset, _ = login("login-before-reset")
    count = stack.mail.count
    _, neutral = browser.request("reset-unknown", "/api/auth/password/reset/request", {"email":unknown})
    require(neutral == {"status":"SENT"} and stack.mail.count == count, "reset-unknown-neutral")

    def reset_token(label):
        browser.request(label, "/api/auth/password/reset/request", {"email":email, "returnTarget":"https://hostile.invalid"})
        link = urlsplit(mail("PASSWORD_RESET")["reset_link"])
        require(link.scheme + "://" + link.netloc == browser.origin and link.path == "/reset-password"
                and not link.query and not link.username and not link.password, "owned-reset-target")
        tokens = parse_qs(link.fragment)
        require(set(tokens) == {"token"} and len(tokens["token"]) == 1, "reset-token-fragment")
        return tokens["token"][0]

    token = reset_token("reset-request-expiry")
    stack.cluster.sql("mcr_auth", f"update password_reset_tokens set expires_at=now()-interval '1 second' where user_id='{user}';")
    browser.request("reset-expired", "/api/auth/password/reset/confirm", {"token":token,"newPassword":replacement}, expected=400)
    before = auth_snapshot(stack.cluster)
    browser.request("reset-cooldown-neutral", "/api/auth/password/reset/request", {"email":email})
    require(auth_snapshot(stack.cluster) == before, "reset-cooldown-does-not-enqueue")
    # Preserve the producer's one-minute cooldown; advance only this owned fixture.
    stack.cluster.sql("mcr_auth", f"update auth_mail_outbox set created_at=now()-interval '61 seconds' where user_id='{user}' and kind='PASSWORD_RESET';")
    token = reset_token("reset-request")
    browser.request("reset-password-too-long", "/api/auth/password/reset/confirm", {"token":token,"newPassword":"é"*65}, expected=400)
    browser.request("reset-confirm-128-utf8-bytes", "/api/auth/password/reset/confirm", {"token":token,"newPassword":replacement})
    browser.request("reset-reused", "/api/auth/password/reset/confirm", {"token":token,"newPassword":replacement}, expected=400)
    browser.request("old-password-rejected", "/api/auth/login", {"email":email,"password":password}, expected=401)
    browser.request("reset-revokes-session", "/api/auth/refresh", cookie=before_reset, expected=401)
    final, _ = login("new-password-login", replacement)
    browser.request("final-logout", "/api/auth/logout", cookie=final, expected=204)
    browser.request("logout-missing-cookie", "/api/auth/logout", expected=204)
    stack.health()
    report.update({"email_lifecycle":"PASS", "refresh_rotation_reuse_revocation":"PASS",
                   "utf8_password_boundaries":"PASS", "safe_reset_target":"PASS",
                   "actual_local_deliveries":len(seen), "canonical_identity_preserved":True,
                   "expiry_fixture":"owned database deadlines; actual JWT clock expiration NOT_RUN"})


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute() or not args.bin_dir.is_absolute():
        parser.error("binary directory and new report must be absolute")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be a new writable file")
    report = {"scope":"BE03 isolated native C03 email auth", "result":"FAIL", "statuses":{},
              "script_sha256":dev.runtime.digest(Path(__file__)), "source_revision":None,
              "browser_cookie_engine":"NOT_RUN", "optional_oauth":"DISABLED_UNTIL_HANDOFF_QUALIFIED",
              "C03_acceptance":"integrator decision", "production":"NOT_RUN", "secrets_recorded":False}
    root = Path(__file__).resolve().parents[3]
    sources = sorted(path for service in ("api-gateway", "auth-service")
                     for path in (root / "backend/services" / service / "src").rglob("*")
                     if path.suffix in (".rs", ".sql"))
    report["source_files_sha256"] = {str(path.relative_to(root)):dev.runtime.digest(path) for path in sources}
    report["contract_inputs_sha256"] = {name:dev.runtime.digest(root / name) for name in (
        "docs/mcr/core-contract.md", "contracts/core/v1/operations.json",
        "contracts/core/v1/core.schema.json", "backend/docs/api/mcr-core.openapi.json")}
    stack = dev.DevStack(args.bin_dir, Path("/opt/homebrew/bin"), Path("/opt/homebrew/bin/redis-server"), Path("/opt/homebrew/bin/openssl"))
    def interrupted(*_args):
        raise KeyboardInterrupt
    previous = {sig:signal.signal(sig, interrupted) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        descriptor = stack.start()
        report["source_revision"] = stack.report.get("source_revision")
        exercise(stack, report, descriptor)
        report["result"] = "PASS"
    except (Exception, KeyboardInterrupt) as error:
        report["failure"] = str(error) if isinstance(error, CheckFailure) else type(error).__name__
    finally:
        try:
            stack.close()
        except BaseException:
            stack.report["cleanup"] = "FAIL"
        report["owned_stack"] = stack.report
        if stack.report.get("cleanup") != "PASS":
            report["result"] = "FAIL"
        for sig, handler in previous.items(): signal.signal(sig, handler)
        dev.write_fd(fd, report)
    print(json.dumps({"result":report["result"], "cleanup":stack.report.get("cleanup"), "checks":len(report["statuses"])}))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
