"""Shared OpenAPI schema and operation builders."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any


OperationAdder = Callable[..., None]


def ref(name: str) -> dict[str, str]:
    return {"$ref": f"#/components/schemas/{name}"}


def string(**kwargs: Any) -> dict[str, Any]:
    return {"type": "string", **kwargs}


def integer(**kwargs: Any) -> dict[str, Any]:
    return {"type": "integer", **kwargs}


def number(**kwargs: Any) -> dict[str, Any]:
    return {"type": "number", **kwargs}


def boolean(**kwargs: Any) -> dict[str, Any]:
    return {"type": "boolean", **kwargs}


def array(items: dict[str, Any], **kwargs: Any) -> dict[str, Any]:
    return {"type": "array", "items": items, **kwargs}


def obj(
    properties: dict[str, Any] | None = None,
    required: list[str] | None = None,
    **kwargs: Any,
) -> dict[str, Any]:
    result: dict[str, Any] = {"type": "object", "properties": properties or {}, **kwargs}
    if required:
        result["required"] = required
    return result


def nullable(schema: dict[str, Any]) -> dict[str, Any]:
    return {"anyOf": [schema, {"type": "null"}]}


def path_parameter(
    name: str,
    description: str,
    fmt: str | None = None,
    schema: dict[str, Any] | None = None,
) -> dict[str, Any]:
    value = schema.copy() if schema else string()
    if fmt:
        value["format"] = fmt
    return {
        "name": name,
        "in": "path",
        "required": True,
        "description": description,
        "schema": value,
    }


def query_parameter(
    name: str,
    description: str,
    schema: dict[str, Any] | None = None,
    required: bool = False,
    example: Any | None = None,
) -> dict[str, Any]:
    parameter: dict[str, Any] = {
        "name": name,
        "in": "query",
        "required": required,
        "description": description,
        "schema": schema or string(),
    }
    if example is not None:
        parameter["example"] = example
    return parameter


def json_body(schema_name: str, required: bool = True) -> dict[str, Any]:
    return {
        "required": required,
        "content": {"application/json": {"schema": ref(schema_name)}},
    }


def header_ref(name: str) -> dict[str, str]:
    return {"$ref": f"#/components/headers/{name}"}


WIRE_HEADER_NAMES = {
    "SetCookie": "Set-Cookie",
    "Location": "Location",
    "CacheControl": "Cache-Control",
    "ContentType": "Content-Type",
    "RetryAfter": "Retry-After",
    "OperationId": "X-Operation-Id",
    "IdempotencyReplayed": "Idempotency-Replayed",
}


def response_headers(*names: str) -> dict[str, Any]:
    headers = {"X-Correlation-Id": header_ref("XCorrelationId")}
    for name in names:
        headers[WIRE_HEADER_NAMES[name]] = header_ref(name)
    return headers


def response(
    description: str,
    schema: str | dict[str, Any] | None = None,
    media_type: str = "application/json",
    headers: tuple[str, ...] = (),
    content: dict[str, Any] | None = None,
) -> dict[str, Any]:
    value: dict[str, Any] = {
        "description": description,
        "headers": response_headers(*headers),
    }
    if content is not None:
        value["content"] = content
    elif schema is not None:
        schema_value = ref(schema) if isinstance(schema, str) else schema
        value["content"] = {media_type: {"schema": schema_value}}
    return value


def async_acceptance_response() -> dict[str, Any]:
    """Document a command that was accepted into the durable Gateway queue."""
    return response(
        "The command was durably queued. Poll the operation status resource at the Location header.",
        "AsyncOperationAccepted",
        headers=(
            "OperationId",
            "CacheControl",
            "RetryAfter",
            "Location",
            "IdempotencyReplayed",
        ),
    )


def error_responses() -> dict[str, Any]:
    return {
        "400": response("Invalid request", "ErrorResponse"),
        "415": response("JSON Content-Type required", "ErrorResponse"),
        "422": response("JSON body does not match the request schema", "ErrorResponse"),
        "401": response("Authentication required or token invalid", "ErrorResponse"),
        "403": response("Authenticated but not permitted", "ErrorResponse"),
        "404": response("Resource not found", "ErrorResponse"),
        "409": response("State conflict", "ErrorResponse"),
        "429": response("Rate limit exceeded", "ErrorResponse"),
        "500": response("Internal error", "ErrorResponse"),
        "503": response(
            "Downstream service unavailable, or busy when Retry-After is present",
            "ErrorResponse",
            headers=("RetryAfter",),
        ),
        "504": response("Downstream request timed out", "ErrorResponse"),
    }


COMMON_PARAMETERS = [
    {"$ref": "#/components/parameters/XCorrelationId"},
    {"$ref": "#/components/parameters/XRequestId"},
    {"$ref": "#/components/parameters/XClientId"},
]

IDEMPOTENCY_KEY = {"$ref": "#/components/parameters/IdempotencyKey"}

OAUTH_STATE_SECURITY = [{"OAuthStateCookie": []}]

BEARER_AND_OAUTH_STATE_SECURITY = [{"BearerAuth": [], "OAuthStateCookie": []}]


def operation(
    *,
    operation_id: str,
    tag: str,
    summary: str,
    success_schema: str | dict[str, Any] | None = None,
    success_status: str = "200",
    success_description: str = "Successful response",
    success_media_type: str = "application/json",
    success_headers: tuple[str, ...] = (),
    success_content: dict[str, Any] | None = None,
    security: str = "bearer",
    parameters: list[dict[str, Any]] | None = None,
    request_body: dict[str, Any] | None = None,
    description: str | None = None,
    deprecated: bool = False,
    extra_responses: dict[str, Any] | None = None,
) -> dict[str, Any]:
    common_errors = error_responses()
    request_content = (request_body or {}).get("content", {})
    if "application/json" not in request_content:
        # Axum's 415/422 extractor responses describe JSON body extraction, not
        # bodyless operations or other media types such as multipart/form-data.
        common_errors.pop("415")
        common_errors.pop("422")
    value: dict[str, Any] = {
        "operationId": operation_id,
        "tags": [tag],
        "summary": summary,
        "parameters": [*COMMON_PARAMETERS, *(parameters or [])],
        "responses": {
            success_status: response(
                success_description,
                success_schema,
                media_type=success_media_type,
                headers=success_headers,
                content=success_content,
            ),
            **common_errors,
            **(extra_responses or {}),
        },
    }
    if description:
        value["description"] = description
    if security == "none":
        value["security"] = []
    elif security == "refresh":
        value["security"] = [{"RefreshCookie": []}]
    elif security == "optional":
        value["security"] = [{}, {"BearerAuth": []}]
    elif security == "oauth_state":
        value["security"] = OAUTH_STATE_SECURITY
    elif security == "bearer_oauth_state":
        value["security"] = BEARER_AND_OAUTH_STATE_SECURITY
    elif security == "admin":
        value["security"] = [{"BearerAuth": []}]
        value["x-required-role"] = "ROLE_ADMIN"
    else:
        value["security"] = [{"BearerAuth": []}]
    if request_body:
        value["requestBody"] = request_body
    if deprecated:
        value["deprecated"] = True
    return value


UUID_PATHS = {
    "user_id": "User UUID",
    "target_user_id": "Target user UUID",
    "request_id": "Follow request UUID",
    "post_id": "Post UUID",
    "notification_id": "Notification UUID",
    "report_id": "Moderation report UUID",
    "contract_id": "Route contract UUID",
    "policy_id": "Policy set UUID",
    "client_id": "Client profile UUID",
    "rollout_id": "Rollout UUID",
    "author_id": "Author UUID",
    "operation_id": "Durable operation UUID",
    "event_id": "Post event UUID",
}


def p(name: str) -> dict[str, Any]:
    return path_parameter(name, UUID_PATHS.get(name, name.replace("_", " ").title()), "uuid")


CURSOR = query_parameter(
    "cursor",
    "Opaque cursor returned by the previous page. Omit for the first page.",
    string(maxLength=512),
)
SEARCH_CURSOR = query_parameter(
    "cursor",
    "Opaque signed cursor returned by a previous page. Reuse only for the same query and viewer scope. Cursors expire; omit to restart from the first page.",
    string(maxLength=512),
)
LIMIT10 = query_parameter(
    "limit", "Maximum number of items", integer(minimum=1, maximum=100, default=10)
)
LIMIT20 = query_parameter(
    "limit", "Maximum number of items", integer(minimum=1, maximum=100, default=20)
)
