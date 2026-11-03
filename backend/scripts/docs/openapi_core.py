#!/usr/bin/env python3
"""Generate the checked-in OpenAPI 3.1 contract for the API Gateway."""

from __future__ import annotations

import argparse
import json
from collections.abc import Callable
from pathlib import Path
from typing import Any

from openapi import auth, platform_docs, post_notification, profile_social
from openapi.common import OperationAdder, integer, operation, string
from openapi.schemas import SCHEMAS


ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "docs/api/openapi.json"
Registration = tuple[str, str, dict[str, Any]]
OperationRegistrar = Callable[[OperationAdder], None]
HTTP_METHOD_ORDER = {"get": 0, "post": 1, "put": 2, "patch": 3, "delete": 4}


def collect_registrations(registrar: OperationRegistrar) -> list[Registration]:
    """Collect and validate one operation family's authored registration order."""
    registrations: list[Registration] = []

    def add(path: str, method: str, **kwargs: Any) -> None:
        registrations.append((path, method.lower(), kwargs))

    registrar(add)
    registration_keys = [
        (path, HTTP_METHOD_ORDER[method]) for path, method, _ in registrations
    ]
    if registration_keys != sorted(registration_keys):
        raise ValueError("operation registrations must be sorted by path and method")
    return registrations


def validate_operation_ids(registrations: list[Registration]) -> None:
    """Reject duplicate operation identifiers before path-map assembly."""
    operation_ids: set[str] = set()
    for _, _, kwargs in registrations:
        operation_id = kwargs["operation_id"]
        if operation_id in operation_ids:
            raise ValueError(f"duplicate operationId: {operation_id}")
        operation_ids.add(operation_id)


def build_document() -> dict[str, Any]:
    """Assemble the complete OpenAPI document in deterministic path order."""
    registrations = [
        *collect_registrations(platform_docs.add_operations),
        *collect_registrations(auth.add_operations),
        *collect_registrations(profile_social.add_operations),
        *collect_registrations(post_notification.add_operations),
    ]
    validate_operation_ids(registrations)
    paths: dict[str, dict[str, Any]] = {}
    for path, method, kwargs in registrations:
        paths.setdefault(path, {})[method.lower()] = operation(**kwargs)

    document = {
        "openapi": "3.1.0",
        "jsonSchemaDialect": "https://json-schema.org/draft/2020-12/schema",
        "info": {
            "title": "Port Rust API Gateway",
            "version": "0.5.0",
            "summary": "REST edge for the Port Rust social backend",
            "description": (
                "Complete checked-in REST contract for the Axum gateway. Every operation documents "
                "its request body, path/query/header inputs, authentication mechanism, success body, "
                "common error envelope, correlation response header, and special cookie/redirect/binary "
                "responses. Authorization is represented by security schemes rather than a duplicate "
                "Authorization header parameter. User-Agent is supplied automatically by the client, "
                "and X-Forwarded-For is infrastructure-owned and must only be injected by a trusted "
                "reverse proxy. Internal service-to-service APIs use protobuf/gRPC and are documented "
                "in proto-reference.md."
            ),
            "license": {"name": "Private repository"},
        },
        "servers": [
            {"url": "http://localhost:8080", "description": "Local runtime"},
            {"url": "https://api.example.com", "description": "Production placeholder"},
        ],
        "tags": [
            {"name": name}
            for name in [
                "Developer",
                "Operations",
                "Authentication",
                "OAuth",
                "Account lifecycle",
                "Profiles",
                "Social graph",
                "Follow requests",
                "Relationship safety",
                "Recommendations",
                "Feed",
                "Posts",
                "Comments",
                "Notifications",
                "Moderation",
                "Admin moderation",
                "Gateway aggregation",
                "Admin governance",
                "Parity",
            ]
        ],
        "paths": dict(sorted(paths.items())),
        "components": {
            "securitySchemes": {
                "BearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "JWT",
                    "description": "RS256 access token issued by auth-service. Enter only the token; Swagger UI adds the Bearer prefix.",
                },
                "RefreshCookie": {
                    "type": "apiKey",
                    "in": "cookie",
                    "name": "refresh_token",
                    "description": "HttpOnly refresh-token cookie with the invariant public name refresh_token. Swagger UI cannot read the value but sends the browser cookie when withCredentials is enabled.",
                },
                "OAuthStateCookie": {
                    "type": "apiKey",
                    "in": "cookie",
                    "name": "oauth_state_{provider}",
                    "description": "One-time HttpOnly OAuth state cookie bound to the provider path parameter. The provider-to-cookie mapping is declared by x-provider-cookie-names.",
                    "x-provider-cookie-names": {
                        "google": "oauth_state_google",
                        "apple": "oauth_state_apple",
                        "x": "oauth_state_x",
                    },
                },
            },
            "parameters": {
                "XCorrelationId": {
                    "name": "X-Correlation-Id",
                    "in": "header",
                    "required": False,
                    "description": "Optional end-to-end correlation identifier. If omitted, the Gateway generates one and always echoes it in the response.",
                    "schema": string(maxLength=128),
                    "example": "01J5PORTSOCIALREQUEST000001",
                },
                "XRequestId": {
                    "name": "X-Request-Id",
                    "in": "header",
                    "required": False,
                    "description": "Optional client request identifier used by mutation audit, deletion, and recommendation workflows. X-Correlation-Id is used as a fallback where supported.",
                    "schema": string(maxLength=128),
                    "example": "req-7e15f292-8bf1-4ddb-aec9-b4786ac0bb07",
                },
                "XClientId": {
                    "name": "X-Client-Id",
                    "in": "header",
                    "required": False,
                    "description": "Optional registered Gateway client-profile identifier recorded in bounded telemetry.",
                    "schema": string(maxLength=128),
                    "example": "web-spa",
                },
                "IdempotencyKey": {
                    "name": "Idempotency-Key",
                    "in": "header",
                    "required": False,
                    "description": "Optional key for the explicitly supported owner mutation routes. Reuse only for the identical authenticated command; keys are limited to 1–128 ASCII graphic characters and replay lasts 24 hours.",
                    "schema": string(minLength=1, maxLength=128, pattern="^[!-~]{1,128}$"),
                },
            },
            "headers": {
                "XCorrelationId": {
                    "description": "Correlation identifier generated or accepted by the Gateway.",
                    "schema": string(maxLength=128),
                },
                "SetCookie": {
                    "description": "One or more Set-Cookie values. Authentication cookies are HttpOnly; OAuth operations may also set or clear a state cookie.",
                    "schema": string(),
                },
                "Location": {
                    "description": "Operation-status resource or OAuth provider redirect URI.",
                    "schema": string(format="uri"),
                },
                "CacheControl": {
                    "description": "Response cache policy.",
                    "schema": string(example="no-store"),
                },
                "ContentType": {
                    "description": "Stored avatar media type.",
                    "schema": string(enum=["image/png", "image/jpeg", "image/webp", "image/gif"]),
                },
                "RetryAfter": {
                    "description": "Seconds to wait before polling a queued operation or retrying after service saturation.",
                    "schema": integer(minimum=1),
                },
                "OperationId": {
                    "description": "Gateway-assigned identifier for the durable mutation intent.",
                    "schema": string(format="uuid"),
                },
                "IdempotencyReplayed": {
                    "description": "Present with value true when the response reuses an existing operation or durable owner receipt.",
                    "schema": string(enum=["true"]),
                },
            },
            "schemas": SCHEMAS,
        },
    }
    validate_document(document)
    return document


def validate_document(document: dict[str, Any]) -> None:
    """Reject ambiguous operation identifiers and unstable path ordering."""
    paths = document["paths"]
    if list(paths) != sorted(paths):
        raise ValueError("paths must be sorted for deterministic output")

    operation_ids: set[str] = set()
    for path_item in paths.values():
        for value in path_item.values():
            operation_id = value["operationId"]
            if operation_id in operation_ids:
                raise ValueError(f"duplicate operationId: {operation_id}")
            operation_ids.add(operation_id)


def build_content() -> str:
    """Serialize the validated contract with stable JSON formatting."""
    return json.dumps(build_document(), indent=2, ensure_ascii=False) + "\n"


def main(argv: list[str] | None = None) -> int:
    """Check or write the checked-in OpenAPI contract."""
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail when the checked-in contract has drifted",
    )
    args = parser.parse_args(argv)
    document = build_document()
    content = json.dumps(document, indent=2, ensure_ascii=False) + "\n"
    operation_count = sum(len(value) for value in document["paths"].values())

    if args.check:
        existing = OUTPUT.read_text(encoding="utf-8") if OUTPUT.exists() else ""
        if existing != content:
            raise SystemExit(
                "docs/api/openapi.json is stale; run scripts/docs/generate-openapi.py"
            )
        print(f"OpenAPI generation check passed: {operation_count} operations")
        return 0

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(content, encoding="utf-8")
    print(
        f"generated {OUTPUT.relative_to(ROOT)} with {operation_count} operations"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
