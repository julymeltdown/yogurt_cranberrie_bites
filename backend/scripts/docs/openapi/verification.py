"""Public verification resend contract; existing operation families stay unchanged."""

from __future__ import annotations

from copy import deepcopy
from typing import Any

from .common import json_body, obj, operation, response, string

PATH = "/auth/email/resend"


def extend_document(core: dict[str, Any]) -> dict[str, Any]:
    document = deepcopy(core)
    if PATH in document["paths"]:
        raise ValueError("verification route already registered")
    schemas = document["components"]["schemas"]
    for name in ("ResendVerificationRequest", "ResendVerificationResponse"):
        if name in schemas:
            raise ValueError(f"verification schema already registered: {name}")
    schemas["ResendVerificationRequest"] = obj(
        {"email": string(format="email", minLength=1, maxLength=254)},
        ["email"],
        additionalProperties=False,
    )
    schemas["ResendVerificationResponse"] = obj(
        {"status": string(enum=["ACCEPTED"])}, ["status"], additionalProperties=False
    )
    value = operation(
        operation_id="resendVerificationEmail",
        tag="Authentication",
        summary="Request a new email verification challenge",
        request_body=json_body("ResendVerificationRequest"),
        success_schema="ResendVerificationResponse",
        success_status="202",
        success_description="Request handled; does not confirm account existence or delivery",
        security="none",
        description=(
            "Public Gateway endpoint. Uses authenticated internal gRPC; no browser service token, "
            "Bearer token or cookie is required. JSON body is limited to 4096 bytes. Pending, "
            "unknown, active, deleted and account-quota-limited emails have the same 202 body. "
            "Auth permits one verification intent per minute and at most 20 per rolling day, "
            "including signup. Edge quota can return 429 independently. ACCEPTED is not proof "
            "of delivery. No automatic retry or Idempotency-Key replay is supported. "
            "Timeout can occur after the downstream request was accepted."
        ),
        extra_responses={
            "413": response("Request body exceeds 4096 bytes", "ErrorResponse"),
            "502": response("Invalid verification service response", "ErrorResponse"),
        },
    )
    for status in ("401", "403", "404", "409"):
        value["responses"].pop(status, None)
    for result in value["responses"].values():
        result["headers"]["Cache-Control"] = {
            "description": "Verification responses must not be stored by caches.",
            "schema": string(enum=["no-store"]),
        }
    value["x-max-body-bytes"] = 4096
    document["paths"][PATH] = {"post": value}
    document["paths"] = dict(sorted(document["paths"].items()))
    return document
