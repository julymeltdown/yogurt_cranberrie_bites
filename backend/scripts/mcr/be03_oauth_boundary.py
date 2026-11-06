#!/usr/bin/env python3
"""Mounted BE03 negative-boundary probes, not a resource allocator.

A separately qualified, pinned controller owns acceptance, both live guards,
the actual candidate, read leases, snapshots, cleanup and FULL. No such adapter
is supplied or qualified by this source-only increment. Its absence fails;
there is no disabled-route, offline, mock-success or old-email fallback.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import http.client
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
from urllib.parse import parse_qsl, urlencode, urlsplit

sys.dont_write_bytecode = True
TASK = "BE03-OAUTH-BOUNDARY-12CC"
OWNER = "01a19f83-12cc-7f21-8ad3-55749f012d9b"
MAX_RESPONSE = 65536
RETURN_TARGETS = {"/world", "/feed", "/me", "/meet/people"}


class BoundaryFailure(Exception):
    """Only fixed, noncredential labels are emitted."""


def require(condition, label):
    if not condition:
        raise BoundaryFailure(label)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def private_json(path):
    path = Path(path)
    info = path.lstat()
    require(path.is_absolute() and stat.S_ISREG(info.st_mode)
            and info.st_uid == os.getuid() and info.st_mode & 0o077 == 0,
            "private-context-required")
    require(info.st_size <= 262144, "context-size")
    return json.loads(path.read_bytes())


def one(headers, key):
    values = [v for k, v in headers if k.lower() == key.lower()]
    require(len(values) == 1, "unique-response-header")
    return values[0]


def unique_query(value):
    pairs = parse_qsl(value, keep_blank_values=True, strict_parsing=True)
    require(len({key for key, _ in pairs}) == len(pairs), "duplicate-query-field")
    return dict(pairs)


class Gateway:
    """Direct loopback transport: no redirects, proxies or provider requests."""
    def __init__(self, origin, fence):
        parsed = urlsplit(origin)
        require(parsed.scheme == "http" and parsed.hostname in {"127.0.0.1", "::1"}
                and parsed.port is not None and not parsed.username
                and not parsed.password and parsed.path in {"", "/"}
                and not parsed.query and not parsed.fragment, "owned-loopback-origin")
        self.host, self.port, self.fence = parsed.hostname, parsed.port, fence

    def get(self, path, cookie=None):
        return self.request("GET", path, cookie)

    def request(self, method, path, cookie=None, origin=None):
        require(path.startswith("/") and not path.startswith("//")
                and "\r" not in path and "\n" not in path, "local-request-path")
        self.fence()
        connection = http.client.HTTPConnection(self.host, self.port, timeout=3)
        try:
            headers = {"Accept": "application/json"}
            if cookie is not None:
                headers["Cookie"] = cookie
            if method == "POST":
                require(bool(origin), "explicit-browser-origin")
                headers.update({"Origin": origin, "X-MCR-Intent": "browser",
                                "Content-Type": "application/json"})
            require(method in {"GET", "POST"}, "bounded-http-method")
            connection.request(method, path, body="{}" if method == "POST" else None, headers=headers)
            response = connection.getresponse()
            body = response.read(MAX_RESPONSE + 1)
            require(len(body) <= MAX_RESPONSE, "bounded-response")
            result = response.status, response.getheaders(), body
        finally:
            connection.close()
        self.fence()
        return result


def protected_response(headers):
    require("no-store" in one(headers, "cache-control").lower(), "no-store-required")
    require(one(headers, "referrer-policy").lower() == "no-referrer", "no-referrer-required")


def authorize(binding, gateway, return_target, supplied_values=None):
    provider = binding.profile["provider"]
    require(provider in {"google", "apple", "x"}, "known-provider")
    pairs = [("returnTo", value) for value in (supplied_values if supplied_values is not None else [return_target])]
    query = urlencode(pairs)
    status, headers, _ = gateway.get(f"/auth/oauth/{provider}/authorize?{query}")
    require(status == 302, "actual-mounted-authorize-302-required")
    protected_response(headers)
    location = urlsplit(one(headers, "location"))
    require(location.scheme in {"http", "https"} and bool(location.netloc)
            and not location.username and not location.password and not location.fragment,
            "provider-authorization-location")
    require((location.scheme, location.netloc, location.path)
            == tuple(binding.profile["authorization_endpoint"]), "pinned-authorization-endpoint")
    params = unique_query(location.query)
    state = params.get("state", "")
    require(bool(state.strip()), "actual-authorize-state")
    # The producer profile specifies its chosen state/return cookie encoding.
    # Its decoder must verify the exact cookie and target binding; do not assume
    # that the legacy raw-state cookie already binds a return destination.
    cookie = binding.state_cookie(headers, state, return_target)
    require(isinstance(cookie, str) and bool(cookie) and "\r" not in cookie
            and "\n" not in cookie, "qualified-state-cookie")
    binding.fence()
    binding.assert_owned_state(provider, state)
    binding.fence()
    return provider, state, cookie


def observation(binding):
    binding.fence()
    snapshot = binding.observe_no_exchange_boundary()
    binding.fence()
    require(snapshot is not None, "owned-observation-required")
    return snapshot


def unchanged(binding, before):
    after = observation(binding)
    require(after == before, "exchange-identity-session-effect")


def invalid_state(binding, gateway):
    provider, state, cookie = authorize(binding, gateway, binding.profile["default_target"])
    before = observation(binding)
    base = f"/auth/oauth/{provider}/callback?"
    # This inert code is deliberately never submitted to a provider. A provider
    # exchange is a failure, irrespective of whether it rejects the code.
    code = "mcr-invalid-state-must-never-exchange"
    cases = [
        (urlencode({"code": code, "state": state}), None),
        (urlencode({"code": code, "state": ""}), cookie),
        (urlencode({"code": code, "state": state + "-mismatch"}), cookie),
        (urlencode([("code", code), ("state", state), ("state", state)]), cookie),
        (urlencode({"code": code, "state": state}), cookie + "; " + cookie),
    ]
    name, value = cookie.split("=", 1)
    fields = value.split(".")
    require(len(fields) == 5 and fields[0] == "v1", "selected-bound-cookie-format")
    fields[3] = base64.urlsafe_b64encode(b"/me" if binding.profile["default_target"] != "/me" else b"/feed").decode().rstrip("=")
    cases.append((urlencode({"code": code, "state": state}), name + "=" + ".".join(fields)))
    for query, supplied_cookie in cases:
        status, headers, _ = gateway.get(base + query, supplied_cookie)
        require(status == 400, "mounted-invalid-state-400-required")
        protected_response(headers)
        require(not any(k.lower() in {"location", "set-cookie"} for k, _ in headers),
                "invalid-state-must-not-redirect-or-alter-cookie")
        unchanged(binding, before)
    return {"case": "invalid-state", "requests": len(cases), "scope": binding.profile["scope"]}


def denial_return(binding, gateway):
    profile = binding.profile
    # Wire names/allowed targets must come from the designated contract writer,
    # not this harness. Query equality also rejects credential/provider text
    # leakage and any unrecognized or duplicate redirect field.
    require(profile.get("handoff_kind") == "cookie-only-fixed-callback-303",
            "selected-handoff-profile-required")
    require(profile["default_target"] == "/world", "selected-default-target")
    inputs = [[target] for target in sorted(RETURN_TARGETS)] + [
        [], ["https://example.invalid"], ["//example.invalid"], ["\\example.invalid"],
        ["/%2f%2fexample.invalid"], ["/world/../me"], ["/world?next=/me"],
        ["/world#secret"], ["/feed", "/me"],
    ]
    for supplied in inputs:
        target = supplied[0] if len(supplied) == 1 and supplied[0] in RETURN_TARGETS else "/world"
        provider, state, cookie = authorize(binding, gateway, target, supplied)
        before = observation(binding)
        query = urlencode({"state": state, "error": "access_denied",
                           "error_description": "mcr-untrusted-provider-text",
                           "returnTo": "https://example.invalid/callback-override"})
        status, headers, _ = gateway.get(f"/auth/oauth/{provider}/callback?{query}", cookie)
        require(status == 303, "mounted-denial-303-required")
        assert_handoff(headers, "denied", target)
        binding.fence()
        binding.assert_denial_cookies(headers)
        binding.fence()
        unchanged(binding, before)
    return {"case": "denial-return", "requests": len(inputs), "scope": profile["scope"]}


def assert_handoff(headers, status, target):
    protected_response(headers)
    location = urlsplit(one(headers, "location"))
    require(not location.scheme and not location.netloc and not location.fragment
            and location.path == "/auth/callback", "fixed-local-callback")
    expected = {"status": status, "returnTo": target}
    actual_query = unique_query(location.query)
    require(not ({key.lower() for key in actual_query}
                 & {"code", "state", "token", "access_token", "refresh_token", "id_token"})
            and "mcr-untrusted-provider-text" not in location.query,
            "no-callback-credential-or-provider-text")
    require(actual_query == expected,
            "frozen-denial-wire-profile")


def success_handoff(binding, gateway):
    require(binding.profile.get("handoff_kind") == "cookie-only-fixed-callback-303",
            "selected-handoff-profile-required")
    provider, state, cookie = authorize(binding, gateway, "/feed")
    binding.fence()
    # The controller must own an actually configured private provider and its
    # issuance/exchange observations. No fabricated external-provider code or
    # credentials, fallback provider or direct user/session insertion qualifies.
    code, expected_identity = binding.issue_owned_provider_code(provider, state)
    binding.fence()
    require(isinstance(code, str) and bool(code) and expected_identity is not None,
            "actual-owned-provider-issuance")
    status, headers, body = gateway.get(f"/auth/oauth/{provider}/callback?" + urlencode({
        "code": code, "state": state, "returnTo": "https://example.invalid"}), cookie)
    require(status == 303 and not body, "cookie-only-success-303")
    assert_handoff(headers, "success", "/feed")
    binding.fence()
    refresh_cookie = binding.assert_success_cookies(headers)
    binding.fence()
    status, headers, body = gateway.request("POST", "/api/auth/refresh", refresh_cookie,
                                            binding.profile["browser_origin"])
    require(status == 200, "actual-cookie-refresh-required")
    protected_response(headers)
    binding.fence()
    binding.assert_canonical_refresh(body, headers, expected_identity)
    binding.fence()
    return {"case": "success-handoff", "requests": 2, "scope": binding.profile["scope"],
            "frontend_memory_admission_qualification": "NOT_RUN"}


def run(context_path, case):
    context = private_json(context_path)
    require(context.get("task") == TASK and context.get("owner") == OWNER,
            "fresh-oauth-task-context")
    require(context.get("allocation_id") and context.get("allocation_id")
            != "BE03-EMAIL-NATIVE02-0329", "fresh-allocation-required")
    require(context.get("runner_sha256") == sha(__file__), "pinned-runner")
    adapter = Path(context["runtime_adapter_path"])
    require(adapter.is_absolute() and adapter.is_file()
            and sha(adapter) == context["runtime_adapter_sha256"], "pinned-runtime-adapter")
    # This source does NOT provide the adapter or accept its resource book.
    # The future qualified controller must already own this child process and
    # establish both guards before starting it. open_boundary rechecks the
    # exact acceptance/FULL tuple, deadlines, process membership and SQL lease.
    spec = importlib.util.spec_from_file_location("be03_allocated_boundary", adapter)
    require(spec is not None and spec.loader is not None, "runtime-adapter-loader")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    binding = module.open_boundary(context)
    binding.fence()
    require(binding.profile["scope"] in {"controlled-private-fixture", "public-gateway"},
            "explicit-evidence-scope")
    require(binding.profile.get("contract_revision") and binding.profile.get("producer_revision"),
            "pinned-producer-contract")
    gateway = Gateway(binding.gateway_origin, binding.fence)
    result = {"invalid-state": invalid_state, "denial-return": denial_return,
              "success-handoff": success_handoff}[case](binding, gateway)
    binding.fence()
    binding.write_redacted_case_report(result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--context", required=True)
    parser.add_argument("--case", choices=("invalid-state", "denial-return", "success-handoff"), required=True)
    args = parser.parse_args()
    try:
        run(args.context, args.case)
    except BoundaryFailure as error:
        print(str(error), file=sys.stderr)
        return 1
    except Exception:
        # Native HTTP and provider errors can carry secrets. Preserve detailed
        # diagnostics only in the controller's explicitly private evidence.
        print("boundary-prerequisite-or-runtime-failure", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
