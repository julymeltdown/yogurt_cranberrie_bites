"""Actual public C03 probe for a caller-owned isolated runtime and real tokens.

Import run_pair() from the launcher after real signup/Profile/bootstrap. This
helper creates no accounts, rows, grants, servers or successful fallback. It
never retries a mutation or emits tokens/private HTTP diagnostics. HTTP success
alone does not prove native owner settlement; the launcher must inspect that.
"""
from __future__ import annotations

import json
import urllib.error
import urllib.parse
import urllib.request
import uuid


class ProbeFailure(RuntimeError):
    """Closed failure; never includes private response bytes or credentials."""


def require(condition, message):
    if not condition:
        raise ProbeFailure(message)


def canonical(value):
    try:
        parsed = uuid.UUID(value)
    except (ValueError, TypeError, AttributeError):
        raise ProbeFailure("canonical nonzero identity required") from None
    require(parsed.int != 0 and str(parsed) == value, "canonical nonzero identity required")
    return value


def closed_object(pairs):
    result = {}
    for name, value in pairs:
        require(name not in result, "duplicate public response field")
        result[name] = value
    return result


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, url):
        return None


class Client:
    def __init__(self, base_url, event, actor, token, origin):
        parsed = urllib.parse.urlsplit(base_url)
        require(parsed.scheme in {"http", "https"} and parsed.hostname
                and not parsed.username and not parsed.password
                and not parsed.query and not parsed.fragment
                and parsed.path in {"", "/"}, "explicit public Gateway origin required")
        require(isinstance(token, str) and token and "\n" not in token and "\r" not in token,
                "actual signed-in token required")
        self.base = base_url.rstrip("/")
        self.event, self.actor = canonical(event), canonical(actor)
        self.token, self.origin = token, origin
        self.steps = []

    def request(self, label, method="GET", payload=None, avatar=False, status=200,
                authenticated=True):
        headers = {"Accept": "application/json", "Cache-Control": "no-cache"}
        if authenticated:
            headers["Authorization"] = "Bearer " + self.token
        if method != "GET":
            headers.update({"Origin": self.origin, "X-MCR-Intent": "browser",
                            "Content-Type": "application/json"})
        body = None if payload is None else json.dumps(payload, separators=(",", ":")).encode()
        target = self.base + f"/api/concert/events/{self.event}/me" + ("/avatar" if avatar else "")
        request = urllib.request.Request(target, data=body, headers=headers, method=method)
        # A fresh client per call proves reload without a client-side state/cache.
        try:
            response = urllib.request.build_opener(NoRedirect()).open(request, timeout=5)
        except urllib.error.HTTPError as error:
            response = error
        except (OSError, ValueError):
            raise ProbeFailure("public HTTP transport unavailable") from None
        with response:
            code, actual_headers = response.code, response.headers
            raw = response.read(32769)
        require(code == status, f"{label}: expected HTTP {status}, received {code}")
        require(len(raw) <= 32768, f"{label}: oversized public response")
        require(actual_headers.get("Cache-Control") == "no-store"
                and actual_headers.get("Pragma") == "no-cache"
                and actual_headers.get("X-Content-Type-Options") == "nosniff"
                and actual_headers.get("ETag") is None
                and actual_headers.get("Last-Modified") is None,
                f"{label}: missing response privacy protection")
        try:
            value = json.loads(raw, object_pairs_hook=closed_object)
        except (ValueError, UnicodeError):
            raise ProbeFailure(f"{label}: invalid public JSON") from None
        require(isinstance(value, dict), f"{label}: object response required")
        self.steps.append({"operation": label, "http_status": code})
        return value

    def document(self, label):
        value = self.request(label)
        require(value.get("event_id") == self.event and value.get("user_id") == self.actor
                and value.get("profile", {}).get("user_id") == self.actor,
                f"{label}: response crossed authenticated principal/event")
        require(type(value.get("revision")) is int and value["revision"] > 0,
                f"{label}: valid participant revision required")
        return value

    def exercise(self):
        first = self.document("initial GET")
        avatar = first.get("profile", {}).get("avatar", {})
        config = avatar.get("config", {})
        require(config.get("contract_version") == "1.1.0"
                and config.get("manifest_id") == "mcr_selected_static_20261103"
                and config.get("representation") == "whole_character"
                and type(avatar.get("revision")) is int,
                "actual selected-pack avatar required")
        selected = dict(config)
        selected["preset_id"] = ("photo01_front_02" if config.get("preset_id") == "photo01_front_01"
                                 else "photo01_front_01")
        saved_avatar = self.request("avatar PUT", "PUT",
                                    {"expected_revision": avatar["revision"], "config": selected}, True)
        require(saved_avatar.get("config") == selected
                and saved_avatar.get("revision") == avatar["revision"] + 1,
                "avatar PUT did not persist selected-pack CAS")
        preferences = {"theme": "DANGER", "low_power": True,
                       "reduced_motion": True, "sound_enabled": False}
        saved = self.request("preferences PATCH", "PATCH",
                             {"expected_revision": first["revision"], "preferences": preferences})
        require(saved.get("revision") == first["revision"] + 1
                and saved.get("preferences") == preferences, "preference CAS did not persist")
        display = "C03 runtime " + self.actor[:8]
        full = self.request("canonical Profile PATCH", "PATCH",
                            {"expected_revision": saved["revision"], "display_name": display, "bio": None})
        require(full.get("revision") == saved["revision"] + 1
                and full.get("profile", {}).get("display_name") == display
                and full["profile"].get("bio", "missing") is None,
                "full-profile CAS did not persist canonical fields")
        reloaded = self.document("fresh reload GET")
        require(reloaded == full and reloaded.get("preferences") == preferences
                and reloaded["profile"].get("avatar") == saved_avatar,
                "fresh reload lost canonical Profile/avatar/preferences")
        stale = self.request("stale CAS refusal", "PATCH",
                             {"expected_revision": first["revision"], "bio": "must not persist"}, status=409)
        require(stale.get("details", {}).get("reason") == "VERSION_CONFLICT",
                "stale CAS did not return closed VERSION_CONFLICT")
        denied = self.request("unauthenticated refusal", authenticated=False, status=401)
        require(denied.get("details", {}).get("reason") == "AUTHENTICATION_REQUIRED",
                "anonymous request did not return closed authentication refusal")
        final = self.document("reload after refused writes")
        require(final == reloaded, "refused request changed saved participant state")
        return final


def run_pair(base_url, event_id, origin, principals):
    """Two actual {user_id, access_token} dictionaries from isolated real login.

    Caller must first bootstrap both users with the actual selected product
    release. On failure inspect private launcher diagnostics, never retry blindly.
    Tokens and returned full profiles are intentionally absent from this report.
    """
    require(len(principals) == 2, "two actual signed-in principals required")
    clients = [Client(base_url, event_id, p["user_id"], p["access_token"], origin)
               for p in principals]
    require(clients[0].actor != clients[1].actor, "distinct actual principals required")
    expected = [client.exercise() for client in clients]
    for client, saved in zip(clients, expected):
        require(client.document("cross-principal isolation reload") == saved,
                "another principal changed the saved participant state")
    return {"state": "PASS", "scope": "actual public signed-in C03 HTTP save/reload",
            "principals": [{"user_id": client.actor, "steps": client.steps}
                           for client in clients],
            "owner_native_settlement": "NOT_CHECKED_BY_HTTP_PROBE",
            "browser_ui": "NOT_RUN"}
