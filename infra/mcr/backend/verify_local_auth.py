#!/usr/bin/env python3
"""Exercise C03 email routes and actual local mail on an already-owned dev stack; not C03 acceptance."""
import argparse
import http.client
import json
import os
from pathlib import Path
import secrets
import sys
import time
from urllib.parse import parse_qs, urlsplit

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "backend/scripts/mcr"))
import dev_stack as dev


def exercise(ready_file):
    status = dev.control_request(ready_file, "status")
    if status["state"] != "READY" or len(status["services"]) != 6:
        raise ValueError("owned six-service stack is not ready")
    origin = dev.local_origin(status["gateway_url"])
    descriptor = json.loads(ready_file.read_bytes())
    # Derive the inbox from the validated root, not an arbitrary descriptor path.
    inbox = Path(descriptor["root"]) / "mail"
    parsed = urlsplit(origin)
    email = "mcr-dev-" + secrets.token_hex(8) + "@example.test"
    password = "Mcr!" + secrets.token_hex(16)
    statuses = {}

    def request(label, path, payload=None, cookie=None, expected=200):
        connection = http.client.HTTPConnection(parsed.hostname, parsed.port, timeout=8)
        headers = {"Content-Type": "application/json", "Origin": origin, "X-MCR-Intent": "browser"}
        if cookie:
            headers["Cookie"] = cookie
        try:
            connection.request("POST", path, body=json.dumps(payload or {}), headers=headers)
            response = connection.getresponse()
            body, response_headers = response.read(65537), dict(response.getheaders())
            statuses[label] = response.status
            if response.status != expected or len(body) > 65536:
                raise ValueError("inherited auth response did not meet seam check")
            return response_headers, json.loads(body) if body else None
        finally:
            connection.close()

    def mail(kind):
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            for path in inbox.glob("*.json"):
                if path.is_symlink() or path.stat().st_mode & 0o077:
                    raise ValueError("capture is not a private regular file")
                value = json.loads(path.read_bytes())
                if value["kind"] == kind and value["payload"]["email"] == email:
                    return value["payload"]
            time.sleep(0.1)
        raise ValueError("actual Auth outbox delivery did not arrive")

    request("signup", "/api/auth/email/signup", {"email": email, "password": password})
    delivery = mail("VERIFY_EMAIL")
    request("verify", "/api/auth/email/verify", {"email": email, "code": delivery["verification_code"]})
    headers, data = request("login", "/api/auth/login", {"email": email, "password": password})
    if not data["accessToken"] or data["refreshToken"] != "" or "HttpOnly" not in headers.get("set-cookie", ""):
        raise ValueError("inherited token/cookie seam mismatch")
    cookie = headers["set-cookie"].split(";", 1)[0]
    headers, data = request("refresh", "/api/auth/refresh", cookie=cookie)
    request("logout", "/api/auth/logout", cookie=headers["set-cookie"].split(";", 1)[0], expected=204)
    request("reset_request", "/api/auth/password/reset/request", {"email": email})
    link = urlsplit(mail("PASSWORD_RESET")["reset_link"])
    if f"{link.scheme}://{link.netloc}" != origin or link.path != "/reset-password" or link.query:
        raise ValueError("reset link does not bind the owned local application")
    # Source PasswordResetLinkBaseUrl deliberately puts the token in a fragment.
    token = parse_qs(link.fragment)["token"][0]
    request("reset_confirm", "/api/auth/password/reset/confirm", {"token": token, "newPassword": "Mcr!" + secrets.token_hex(16)})
    return {"result": "PASS", "scope": "inherited auth/local delivery seam only", "C03_acceptance": "NOT_RUN",
            "statuses": statuses, "actual_local_deliveries": 2, "secret_values_recorded": False,
            "script_sha256": dev.runtime.digest(Path(__file__))}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ready-file", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    if not args.report.is_absolute():
        parser.error("report must be an absolute new file")
    try:
        fd = os.open(args.report, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    except OSError:
        parser.error("report must be a new writable file; existing evidence is preserved")
    try:
        result = exercise(args.ready_file)
    except (Exception, KeyboardInterrupt) as error:
        # Credentials, response bodies, mail contents and exception messages are never printed.
        result = {"result": "FAIL", "diagnostic": type(error).__name__, "C03_acceptance": "NOT_RUN"}
    dev.write_fd(fd, result)
    print(json.dumps(result))
    return 0 if result["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
