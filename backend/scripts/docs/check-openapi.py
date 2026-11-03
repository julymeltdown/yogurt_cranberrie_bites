#!/usr/bin/env python3
"""Validate OpenAPI structure and compare it with the Axum gateway router.

The checker validates more than route existence. It extracts handler signatures
from the Gateway sources and verifies JSON/multipart request bodies, query/path
parameters, authentication requirements, common request headers, response
correlation headers, and special cookie/redirect/binary responses.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Final, Iterable

from openapi_bundle import load_document

ROOT = Path(__file__).resolve().parents[2]
OPENAPI_PATH = ROOT / "docs/api/openapi.json"
ROUTER_ROOT = ROOT / "services/api-gateway/src/adapters/in/http"
DTO_ROOT = ROOT / "crates/gateway-application/src/dto"
DOMAIN_ROOT = ROOT / "crates/gateway-domain/src"
HTTP_METHODS = {"get", "post", "put", "patch", "delete"}
COMMON_PARAMETER_REFS = {
    "#/components/parameters/XCorrelationId",
    "#/components/parameters/XRequestId",
    "#/components/parameters/XClientId",
}
COMMON_EXTRACTOR_ERROR_STATUSES: Final[set[str]] = {"400"}
JSON_BODY_EXTRACTOR_ERROR_STATUSES: Final[set[str]] = {"415", "422"}
DEDICATED_ROUTER_HANDLERS: Final[set[tuple[Path, str]]] = {
    (ROUTER_ROOT / "mod.rs", "metrics"),
}
GENERIC_SUCCESS_ALLOWLIST = {
    ("/openapi.json", "get"),
}
EXPECTED_SUCCESS_RESPONSE_SCHEMAS: Final[dict[tuple[str, str], tuple[str, str]]] = {
    ("/admin/account-deletions/{saga_id}/retry", "post"): (
        "202",
        "UserDeletionRetryAcceptedResponse",
    ),
    ("/admin/moderation/content-decisions", "get"): (
        "200",
        "ModerationContentDecisionListResponse",
    ),
    ("/admin/moderation/content-decisions/{report_id}/replay", "post"): (
        "202",
        "ContentDecisionReplayAcceptedResponse",
    ),
    ("/api/follows/requests/{request_id}/accept", "post"): ("200", "FollowActionResponse"),
    ("/api/follows/requests/{request_id}/reject", "post"): ("200", "FollowRequestResponse"),
    ("/api/posts/author/{author_id}", "get"): ("200", "FeedResponse"),
    ("/api/posts/search", "get"): ("200", "FeedResponse"),
    ("/api/posts/trending", "get"): ("200", "FeedResponse"),
    ("/admin/post-events/dead-letters/{event_id}/replay", "post"): (
        "202",
        "DeadLetterReplayAcceptedResponse",
    ),
    ("/auth/email/resend", "post"): ("202", "ResendVerificationResponse"),
}


RESPONSE_SCHEMA_STRUCTS = {
    "SignupResponse": "SignupResponse",
    "ResendVerificationResponse": "ResendVerificationResponse",
    "EmailAvailabilityResponse": "EmailAvailabilityResponse",
    "PasswordResetResponse": "PasswordResetResponse",
    "AuthResponse": "AuthResponse",
    "LinkedProvidersResponse": "LinkedProvidersResponse",
    "ShadowComparisonResponse": "ShadowComparisonResponse",
    "UserDeletionStep": "UserDeletionStepResponse",
    "UserDeletionSaga": "UserDeletionSagaResponse",
    "DeleteAccountResponse": "DeleteAccountResponse",
    "UserDeletionRetryAcceptedResponse": "RetryUserDeletionAcceptedResponse",
    "Profile": "ProfileResponse",
    "ProfileBatchResponse": "ProfileBatchResponse",
    "ProfileSearchResponse": "ProfileSearchResponse",
    "AvatarUploadResponse": "AvatarUploadResponse",
    "DeleteProfileResponse": "DeleteProfileResponse",
    "FollowActionResponse": "FollowActionResponse",
    "FollowEdge": "FollowEdgeResponse",
    "FollowListResponse": "FollowListResponse",
    "FollowCountsResponse": "FollowCountsResponse",
    "FollowStatusResponse": "FollowStatusResponse",
    "FollowRequestResponse": "FollowRequestResponse",
    "FollowRequestPageResponse": "FollowRequestPageResponse",
    "RelationshipPolicyResponse": "RelationshipPolicyResponse",
    "PeopleRecommendation": "PeopleRecommendationResponse",
    "PeopleRecommendationPageResponse": "PeopleRecommendationPageResponse",
    "RecommendationEventResponse": "RecommendationEventResponse",
    "Post": "PostResponse",
    "Comment": "CommentResponse",
    "FeedResponse": "FeedResponse",
    "RankedFeedResponse": "RankedFeedResponse",
    "FeedRecommendationMetadata": "FeedRecommendationMetadataResponse",
    "FeedRecommendationEventResponse": "FeedRecommendationEventResponse",
    "PostDetailResponse": "PostDetailResponse",
    "DeletePostResponse": "DeletePostResponse",
    "PostLikeResponse": "PostLikeResponse",
    "DeadLetterReplayAcceptedResponse": "DeadLetterReplayAcceptedResponse",
    "ContentDecisionReplayAcceptedResponse": "ContentDecisionReplayAcceptedResponse",
    "Notification": "NotificationResponse",
    "NotificationFeedResponse": "NotificationFeedResponse",
    "NotificationMarkAllReadResponse": "NotificationMarkAllReadResponse",
    "ModerationReport": "ModerationReportResponse",
    "ModerationDecisionRecord": "ModerationDecisionRecordResponse",
    "ModerationDecisionResponse": "ModerationDecisionResponse",
    "ModerationReportListResponse": "ModerationReportListResponse",
    "ModerationContentDecisionStatus": "ModerationContentDecisionStatusResponse",
    "ModerationContentDecisionCursor": "ModerationContentDecisionCursorResponse",
    "ModerationContentDecisionListResponse": "ModerationContentDecisionListResponse",
    "RouteContract": "RouteContract",
    "RouteContractVersion": "RouteContractVersion",
    "PolicySet": "PolicySet",
    "ClientProfile": "ClientProfile",
    "RolloutPolicy": "RolloutPolicy",
    "TelemetrySummary": "TelemetrySummary",
    "OperationStatus": "OperationStatus",
    "AggregationResponse": "AggregationResponse",
}

SPECIAL_RESPONSE_HEADERS: dict[tuple[str, str, str], set[str]] = {
    ("/auth/login", "post", "200"): {"Set-Cookie"},
    ("/auth/email/resend", "post", "202"): {"Cache-Control"},
    ("/auth/refresh", "post", "200"): {"Set-Cookie"},
    ("/auth/logout", "post", "204"): {"Set-Cookie"},
    ("/auth/account", "delete", "202"): {"Set-Cookie"},
    ("/auth/oauth/{provider}/authorize", "get", "302"): {"Location", "Set-Cookie"},
    ("/auth/oauth/{provider}/callback", "get", "200"): {"Set-Cookie"},
    ("/auth/oauth/{provider}/link", "post", "200"): {"Set-Cookie"},
    ("/api/profile/avatar/{user_id}", "get", "200"): {"Cache-Control", "Content-Type"},
}


@dataclass(frozen=True)
class QueryField:
    name: str
    required: bool


@dataclass(frozen=True)
class RustField:
    name: str
    required: bool


@dataclass(frozen=True)
class HandlerContract:
    path: str
    method: str
    handler: str
    json_request: str | None
    multipart_request: bool
    query_fields: tuple[QueryField, ...]
    bearer_required: bool
    bearer_optional: bool
    form_request: bool = False


def normalized_path(path: str) -> str:
    return re.sub(r":([A-Za-z_][A-Za-z0-9_]*)", r"{\1}", path)


def is_character_literal(text: str, index: int) -> bool:
    """Distinguish Rust character literals from lifetime apostrophes."""
    if text[index] != "'" or index + 1 >= len(text):
        return False
    if text[index + 1] != "\\":
        return index + 2 < len(text) and text[index + 2] == "'"

    escape_start = index + 2
    if text.startswith("u{", escape_start):
        close_brace = text.find("}", escape_start + 2)
        return close_brace >= 0 and text[close_brace + 1 : close_brace + 2] == "'"
    if text.startswith("x", escape_start):
        return text[escape_start + 3 : escape_start + 4] == "'"
    return text[escape_start + 1 : escape_start + 2] == "'"


def route_blocks(text: str) -> Iterable[str]:
    cursor = 0
    needle = ".route("
    while True:
        start = text.find(needle, cursor)
        if start < 0:
            return
        index = start + len(needle)
        depth = 1
        quote: str | None = None
        escaped = False
        while index < len(text) and depth:
            char = text[index]
            if quote is not None:
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == quote:
                    quote = None
            elif char == '"' or (char == "'" and is_character_literal(text, index)):
                quote = char
            elif char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
            index += 1
        if depth:
            raise ValueError("unbalanced Axum route expression")
        yield text[start + len(needle) : index - 1]
        cursor = index


def balanced_parenthesized(text: str, open_index: int) -> str:
    depth = 1
    index = open_index + 1
    quote: str | None = None
    escaped = False
    while index < len(text) and depth:
        char = text[index]
        if quote is not None:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
        elif char == '"' or (char == "'" and is_character_literal(text, index)):
            quote = char
        elif char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
        index += 1
    if depth:
        raise ValueError("unbalanced function signature")
    return text[open_index + 1 : index - 1]


def handler_signatures(text: str) -> dict[str, str]:
    signatures: dict[str, str] = {}
    for match in re.finditer(r"async\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(", text):
        name = match.group(1)
        open_index = text.find("(", match.start())
        signatures[name] = balanced_parenthesized(text, open_index)
    return signatures


def query_structs(text: str) -> dict[str, tuple[QueryField, ...]]:
    structs: dict[str, tuple[QueryField, ...]] = {}
    for match in re.finditer(r"struct\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{", text):
        name = match.group(1)
        open_index = text.find("{", match.start())
        depth = 1
        index = open_index + 1
        while index < len(text) and depth:
            if text[index] == "{":
                depth += 1
            elif text[index] == "}":
                depth -= 1
            index += 1
        if depth:
            raise ValueError(f"unbalanced struct {name}")
        body = text[open_index + 1 : index - 1]
        fields: list[QueryField] = []
        pending_serde = ""
        for raw_line in body.splitlines():
            line = raw_line.strip()
            if not line:
                continue
            if line.startswith("#[serde("):
                pending_serde = line
                continue
            field_match = re.match(
                r"(?:pub\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([^,]+),?", line
            )
            if not field_match:
                pending_serde = ""
                continue
            field_name, field_type = field_match.groups()
            rename = re.search(r'rename\s*=\s*"([^"]+)"', pending_serde)
            serialized_name = rename.group(1) if rename else field_name
            required = "Option<" not in field_type and "default" not in pending_serde
            fields.append(QueryField(serialized_name, required))
            pending_serde = ""
        if fields:
            structs[name] = tuple(fields)
    return structs




def camel_case(value: str) -> str:
    head, *tail = value.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in tail)


def rust_structs(root: Path) -> dict[str, tuple[RustField, ...]]:
    structs: dict[str, tuple[RustField, ...]] = {}
    for source_path in sorted(root.rglob("*.rs")):
        text = source_path.read_text(encoding="utf-8")
        for match in re.finditer(r"(?:#\[[^\n]+\]\s*)*pub\s+struct\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{", text):
            name = match.group(1)
            declaration = match.group(0)
            rename_all_match = re.search(r'rename_all\s*=\s*"([^"]+)"', declaration)
            rename_all = rename_all_match.group(1) if rename_all_match else None
            open_index = text.find("{", match.start())
            depth = 1
            index = open_index + 1
            while index < len(text) and depth:
                if text[index] == "{":
                    depth += 1
                elif text[index] == "}":
                    depth -= 1
                index += 1
            if depth:
                raise ValueError(f"unbalanced DTO struct {name}")
            body = text[open_index + 1 : index - 1]
            fields: list[RustField] = []
            pending_serde = ""
            for raw_line in body.splitlines():
                line = raw_line.strip()
                if not line:
                    continue
                if line.startswith("#[serde("):
                    pending_serde += line
                    continue
                field_match = re.match(
                    r"pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([^,]+),?", line
                )
                if not field_match:
                    pending_serde = ""
                    continue
                field_name, field_type = field_match.groups()
                rename = re.search(r'rename\s*=\s*"([^"]+)"', pending_serde)
                if rename:
                    serialized_name = rename.group(1)
                elif rename_all == "camelCase":
                    serialized_name = camel_case(field_name)
                else:
                    serialized_name = field_name
                required = "Option<" not in field_type and "default" not in pending_serde
                fields.append(RustField(serialized_name, required))
                pending_serde = ""
            if fields:
                structs[name] = tuple(fields)
    return structs


def source_contracts() -> dict[tuple[str, str], HandlerContract]:
    contracts: dict[tuple[str, str], HandlerContract] = {}
    for source_path in sorted(ROUTER_ROOT.rglob("*.rs")):
        text = source_path.read_text(encoding="utf-8")
        signatures = handler_signatures(text)
        structs = query_structs(text)
        for block in route_blocks(text):
            path_match = re.search(r'"([^"]+)"', block)
            if not path_match:
                continue
            route = normalized_path(path_match.group(1))
            for method, handler in re.findall(
                r"(?:\b|::|\.)(get|post|put|patch|delete)\s*\(\s*([A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*)",
                block,
            ):
                if (source_path, handler) in DEDICATED_ROUTER_HANDLERS:
                    continue
                handler_text = text
                if "::" in handler:
                    # Resolve local child-module handlers instead of silently
                    # treating the module name as a bodyless handler.
                    *modules, name = handler.split("::")
                    module_root = source_path.parent if source_path.stem == "mod" else source_path.with_suffix("")
                    child_path = module_root.joinpath(*modules).with_suffix(".rs")
                    if not child_path.is_file():
                        raise ValueError(f"unresolved route handler {handler}: {child_path}")
                    handler_text = child_path.read_text(encoding="utf-8")
                    signature = handler_signatures(handler_text).get(name, "")
                    if not signature:
                        raise ValueError(f"missing route handler signature: {handler}")
                else:
                    signature = signatures.get(handler, "")
                json_match = re.search(
                    r"Json\s*\([^)]*\)\s*:\s*Json\s*<\s*([A-Za-z_][A-Za-z0-9_:]*)\s*>",
                    signature,
                )
                if json_match is None:
                    json_match = re.search(
                        r":\s*Result\s*<\s*Json\s*<\s*([A-Za-z_][A-Za-z0-9_:]*)\s*>",
                        signature,
                    )
                query_match = re.search(
                    r"Query\s*\([^)]*\)\s*:\s*Query\s*<\s*([A-Za-z_][A-Za-z0-9_:]*)\s*>",
                    signature,
                )
                query_type = query_match.group(1).split("::")[-1] if query_match else None
                contracts[(route, method)] = HandlerContract(
                    path=route,
                    method=method,
                    handler=handler,
                    json_request=(
                        json_match.group(1).split("::")[-1] if json_match else None
                    ),
                    multipart_request="Multipart" in signature,
                    query_fields=structs.get(query_type or "", ()),
                    bearer_required=(
                        "CurrentUser" in signature and "OptionalCurrentUser" not in signature
                    ),
                    bearer_optional="OptionalCurrentUser" in signature,
                    # Raw Request adapters may explicitly bound and validate
                    # bytes before invoking Axum's form decoder. This lexical
                    # evidence checks media only; it does not prove body limits,
                    # field validation or runtime binding.
                    form_request=(
                        bool(re.search(r"\bForm\s*<", signature))
                        or (bool(re.search(r":\s*Request\b", signature))
                            and "axum::Form::<" in handler_text)
                    ),
                )
    return contracts


def contract_operations(document: dict[str, Any]) -> set[tuple[str, str]]:
    return {
        (path, method)
        for path, item in document["paths"].items()
        for method in item
        if method in HTTP_METHODS
    }


def parameter_name(parameter: dict[str, Any]) -> tuple[str | None, str | None, bool | None]:
    if "$ref" in parameter:
        return None, None, None
    return parameter.get("name"), parameter.get("in"), parameter.get("required")


def parameter_refs(operation: dict[str, Any]) -> set[str]:
    return {
        parameter["$ref"]
        for parameter in operation.get("parameters", [])
        if "$ref" in parameter
    }


def inline_parameters(operation: dict[str, Any], location: str) -> dict[str, dict[str, Any]]:
    return {
        parameter["name"]: parameter
        for parameter in operation.get("parameters", [])
        if parameter.get("in") == location
    }


def response_schema(response: dict[str, Any]) -> dict[str, Any] | None:
    for media in response.get("content", {}).values():
        if "schema" in media:
            return media["schema"]
    return None


def schema_ref_name(schema: dict[str, Any] | None) -> str | None:
    if not schema:
        return None
    reference = schema.get("$ref")
    if not reference:
        return None
    return reference.rsplit("/", 1)[-1]


def validate_expected_success_response_schemas(document: dict[str, Any]) -> None:
    """Require the named public operations to retain their exact 2xx response refs."""
    for (path, method), (status, expected_schema) in (
        EXPECTED_SUCCESS_RESPONSE_SCHEMAS.items()
    ):
        operation = document["paths"].get(path, {}).get(method)
        if operation is None:
            raise ValueError(f"{method.upper()} {path} is missing from OpenAPI")
        response_value = operation.get("responses", {}).get(status)
        if response_value is None:
            raise ValueError(f"{method.upper()} {path} is missing success response {status}")
        media = response_value.get("content", {}).get("application/json")
        if media is None:
            raise ValueError(
                f"{method.upper()} {path} response {status} must document application/json"
            )
        actual_schema = schema_ref_name(media.get("schema"))
        if actual_schema != expected_schema:
            raise ValueError(
                f"{method.upper()} {path} response {status} schema mismatch: "
                f"expected={expected_schema}, OpenAPI={actual_schema}"
            )


def response_header_names(response: dict[str, Any]) -> set[str]:
    return set(response.get("headers", {}))


def validate_structure(document: dict[str, Any]) -> None:
    if document.get("openapi") != "3.1.0":
        raise ValueError("OpenAPI version must be 3.1.0")
    if document.get("jsonSchemaDialect") != "https://json-schema.org/draft/2020-12/schema":
        raise ValueError("OpenAPI JSON Schema dialect must be draft 2020-12")

    components = document.get("components", {})
    security_schemes = components.get("securitySchemes", {})
    if "BearerAuth" not in security_schemes:
        raise ValueError("BearerAuth security scheme is required")
    if "RefreshCookie" not in security_schemes:
        raise ValueError("RefreshCookie security scheme is required")
    oauth_state = security_schemes.get("OAuthStateCookie")
    if not oauth_state:
        raise ValueError("OAuthStateCookie security scheme is required")
    expected_provider_cookies = {
        "google": "oauth_state_google",
        "apple": "oauth_state_apple",
        "x": "oauth_state_x",
    }
    if oauth_state.get("x-provider-cookie-names") != expected_provider_cookies:
        raise ValueError("OAuthStateCookie provider mapping is invalid")

    component_parameters = components.get("parameters", {})
    for name in ["XCorrelationId", "XRequestId", "XClientId"]:
        if name not in component_parameters:
            raise ValueError(f"missing reusable parameter: {name}")

    component_headers = components.get("headers", {})
    for name in ["XCorrelationId", "SetCookie", "Location", "CacheControl", "ContentType"]:
        if name not in component_headers:
            raise ValueError(f"missing reusable response header: {name}")

    operation_ids: set[str] = set()
    schemas = components.get("schemas", {})
    for path, path_item in document.get("paths", {}).items():
        declared_path_parameters = set(re.findall(r"{([^}]+)}", path))
        for method, operation in path_item.items():
            if method not in HTTP_METHODS:
                continue
            operation_id = operation.get("operationId")
            if not operation_id:
                raise ValueError(f"{method.upper()} {path} is missing operationId")
            if operation_id in operation_ids:
                raise ValueError(f"duplicate operationId: {operation_id}")
            operation_ids.add(operation_id)
            if not operation.get("tags"):
                raise ValueError(f"{method.upper()} {path} is missing tags")
            if not operation.get("responses"):
                raise ValueError(f"{method.upper()} {path} is missing responses")

            for status in COMMON_EXTRACTOR_ERROR_STATUSES:
                error_response = operation["responses"].get(status)
                if error_response is None:
                    raise ValueError(
                        f"{method.upper()} {path} missing common extractor response {status}"
                    )
                content = error_response.get("content", {})
                if set(content) != {"application/json"}:
                    raise ValueError(
                        f"{method.upper()} {path} extractor response {status} "
                        "must use only application/json"
                    )
                if schema_ref_name(response_schema(error_response)) != "ErrorResponse":
                    raise ValueError(
                        f"{method.upper()} {path} extractor response {status} "
                        "must use ErrorResponse"
                    )

            request_content = operation.get("requestBody", {}).get("content", {})
            has_json_body = "application/json" in request_content
            for status in JSON_BODY_EXTRACTOR_ERROR_STATUSES:
                error_response = operation["responses"].get(status)
                if has_json_body and error_response is None:
                    raise ValueError(
                        f"{method.upper()} {path} missing JSON extractor response {status}"
                    )
                if not has_json_body and error_response is not None:
                    raise ValueError(
                        f"{method.upper()} {path} advertises JSON-only extractor response {status} "
                        "without an application/json request body"
                    )
                if error_response is not None:
                    content = error_response.get("content", {})
                    if set(content) != {"application/json"}:
                        raise ValueError(
                            f"{method.upper()} {path} JSON extractor response {status} "
                            "must use only application/json"
                        )
                    if schema_ref_name(response_schema(error_response)) != "ErrorResponse":
                        raise ValueError(
                            f"{method.upper()} {path} JSON extractor response {status} "
                            "must use ErrorResponse"
                        )

            refs = parameter_refs(operation)
            missing_common = COMMON_PARAMETER_REFS - refs
            if missing_common:
                raise ValueError(
                    f"{method.upper()} {path} missing common header parameters: {sorted(missing_common)}"
                )

            operation_path_parameters = set(inline_parameters(operation, "path"))
            if operation_path_parameters != declared_path_parameters:
                raise ValueError(
                    f"{method.upper()} {path} path parameters differ: "
                    f"declared={declared_path_parameters}, operation={operation_path_parameters}"
                )

            for status, response_value in operation["responses"].items():
                headers = response_header_names(response_value)
                if "X-Correlation-Id" not in headers:
                    raise ValueError(
                        f"{method.upper()} {path} response {status} missing X-Correlation-Id"
                    )

            for status, response_value in operation["responses"].items():
                if not status.startswith("2") and status != "302":
                    continue
                if (path, method) in GENERIC_SUCCESS_ALLOWLIST:
                    continue
                if schema_ref_name(response_schema(response_value)) == "GenericObject":
                    raise ValueError(
                        f"{method.upper()} {path} uses GenericObject for success response"
                    )

    serialized = json.dumps(document)
    for reference in re.findall(r"#/components/schemas/([A-Za-z0-9_]+)", serialized):
        if reference not in schemas:
            raise ValueError(f"unknown schema reference: {reference}")


def has_security_scheme(security: list[dict[str, list[Any]]], scheme: str) -> bool:
    return any(scheme in requirement for requirement in security)


def validate_against_handlers(
    document: dict[str, Any],
    contracts: dict[tuple[str, str], HandlerContract],
    dto_structures: dict[str, tuple[RustField, ...]],
) -> None:
    for key, source in sorted(contracts.items()):
        path, method = key
        operation = document["paths"][path][method]
        request_body = operation.get("requestBody")

        if source.json_request:
            if not request_body:
                raise ValueError(
                    f"{method.upper()} {path} handler {source.handler} accepts JSON but OpenAPI has no requestBody"
                )
            media = request_body.get("content", {}).get("application/json")
            if not media:
                raise ValueError(
                    f"{method.upper()} {path} JSON handler must document application/json"
                )
            actual_schema = schema_ref_name(media.get("schema"))
            if actual_schema != source.json_request:
                raise ValueError(
                    f"{method.upper()} {path} request schema mismatch: "
                    f"source={source.json_request}, OpenAPI={actual_schema}"
                )
            rust_fields = dto_structures.get(source.json_request)
            if rust_fields is None:
                raise ValueError(
                    f"{method.upper()} {path} request DTO {source.json_request} was not parsed"
                )
            openapi_schema = document["components"]["schemas"][actual_schema]
            actual_properties = set(openapi_schema.get("properties", {}))
            expected_properties = {field.name for field in rust_fields}
            if actual_properties != expected_properties:
                raise ValueError(
                    f"{method.upper()} {path} request properties differ: "
                    f"source={sorted(expected_properties)}, OpenAPI={sorted(actual_properties)}"
                )
            actual_required = set(openapi_schema.get("required", []))
            expected_required = {field.name for field in rust_fields if field.required}
            if actual_required != expected_required:
                raise ValueError(
                    f"{method.upper()} {path} request required fields differ: "
                    f"source={sorted(expected_required)}, OpenAPI={sorted(actual_required)}"
                )
        elif source.multipart_request:
            if not request_body or "multipart/form-data" not in request_body.get("content", {}):
                raise ValueError(
                    f"{method.upper()} {path} Multipart handler must document multipart/form-data"
                )
        elif source.form_request:
            if not request_body or "application/x-www-form-urlencoded" not in request_body.get("content", {}):
                raise ValueError(
                    f"{method.upper()} {path} form handler must document application/x-www-form-urlencoded"
                )
        elif request_body:
            raise ValueError(
                f"{method.upper()} {path} documents a requestBody but handler {source.handler} has none"
            )

        query_parameters = inline_parameters(operation, "query")
        source_query = {field.name: field for field in source.query_fields}
        if set(query_parameters) != set(source_query):
            raise ValueError(
                f"{method.upper()} {path} query parameters differ: "
                f"source={sorted(source_query)}, OpenAPI={sorted(query_parameters)}"
            )
        for name, field in source_query.items():
            if bool(query_parameters[name].get("required")) != field.required:
                raise ValueError(
                    f"{method.upper()} {path} query parameter {name} required mismatch"
                )

        security = operation.get("security", [])
        if source.bearer_required and not has_security_scheme(security, "BearerAuth"):
            raise ValueError(f"{method.upper()} {path} must require BearerAuth")
        if source.bearer_optional and not (
            {} in security and has_security_scheme(security, "BearerAuth")
        ):
            raise ValueError(f"{method.upper()} {path} must document optional BearerAuth")


def validate_response_structures(
    document: dict[str, Any], structures: dict[str, tuple[RustField, ...]]
) -> None:
    schemas = document["components"]["schemas"]
    for schema_name, struct_name in RESPONSE_SCHEMA_STRUCTS.items():
        fields = structures.get(struct_name)
        if fields is None:
            raise ValueError(f"response struct {struct_name} was not parsed")
        schema = schemas.get(schema_name)
        if schema is None:
            raise ValueError(f"response schema {schema_name} is missing")
        expected = {field.name for field in fields}
        actual = set(schema.get("properties", {}))
        if expected != actual:
            raise ValueError(
                f"response schema {schema_name} differs from {struct_name}: "
                f"source={sorted(expected)}, OpenAPI={sorted(actual)}"
            )
        # Gateway response structs do not use skip_serializing_if, so Option fields
        # are serialized as explicit nulls and remain required JSON properties.
        required = set(schema.get("required", []))
        if required != expected:
            raise ValueError(
                f"response schema {schema_name} required fields differ from {struct_name}: "
                f"source={sorted(expected)}, OpenAPI={sorted(required)}"
            )


def validate_special_responses(document: dict[str, Any]) -> None:
    for (path, method, status), required_headers in SPECIAL_RESPONSE_HEADERS.items():
        response_value = document["paths"][path][method]["responses"].get(status)
        if not response_value:
            raise ValueError(f"{method.upper()} {path} missing response {status}")
        actual_headers = response_header_names(response_value)
        missing = required_headers - actual_headers
        if missing:
            raise ValueError(
                f"{method.upper()} {path} response {status} missing headers: {sorted(missing)}"
            )

    refresh_security = document["paths"]["/auth/refresh"]["post"].get("security", [])
    logout_security = document["paths"]["/auth/logout"]["post"].get("security", [])
    if not has_security_scheme(refresh_security, "RefreshCookie"):
        raise ValueError("POST /auth/refresh must document RefreshCookie")
    if not has_security_scheme(logout_security, "RefreshCookie"):
        raise ValueError("POST /auth/logout must document RefreshCookie")

    callback_security = document["paths"]["/auth/oauth/{provider}/callback"]["get"].get("security", [])
    if callback_security != [{"OAuthStateCookie": []}]:
        raise ValueError("OAuth callback must require one provider-bound state cookie")
    link_security = document["paths"]["/auth/oauth/{provider}/link"]["post"].get("security", [])
    if link_security != [{"BearerAuth": [], "OAuthStateCookie": []}]:
        raise ValueError("OAuth link must require BearerAuth and one provider-bound state cookie")

    avatar_response = document["paths"]["/api/profile/avatar/{user_id}"]["get"]["responses"]["200"]
    avatar_media = set(avatar_response.get("content", {}))
    required_media = {"image/png", "image/jpeg", "image/webp", "image/gif"}
    if not required_media.issubset(avatar_media):
        raise ValueError(
            f"avatar response must document supported image media types: {sorted(required_media)}"
        )


def main() -> int:
    subprocess.run(
        [sys.executable, str(ROOT / "scripts/docs/generate-openapi.py"), "--check"],
        check=True,
        cwd=ROOT,
    )
    document = load_document(OPENAPI_PATH)
    contracts = source_contracts()
    dto_structures = rust_structs(DTO_ROOT)
    response_structures = {**dto_structures, **rust_structs(DOMAIN_ROOT)}
    validate_structure(document)
    source = set(contracts)
    contract = contract_operations(document)
    missing = sorted(source - contract)
    stale = sorted(contract - source)
    if missing or stale:
        if missing:
            print("Gateway operations missing from OpenAPI:", file=sys.stderr)
            for path, method in missing:
                print(f"  {method.upper()} {path}", file=sys.stderr)
        if stale:
            print("OpenAPI operations with no Gateway route:", file=sys.stderr)
            for path, method in stale:
                print(f"  {method.upper()} {path}", file=sys.stderr)
        return 1
    validate_against_handlers(document, contracts, dto_structures)
    validate_response_structures(document, response_structures)
    validate_special_responses(document)
    validate_expected_success_response_schemas(document)
    print(f"OpenAPI route and payload coverage passed: {len(contract)} operations")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
