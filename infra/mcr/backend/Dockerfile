# syntax=docker/dockerfile:1
# Repository-root context: docker build -f infra/mcr/backend/Dockerfile --build-arg SERVICE=auth-service .
# A packaging candidate, not an executed/qualified production image.
ARG BUILDER_IMAGE=rust:1.88.0-bookworm
ARG RUNTIME_IMAGE=debian:bookworm-slim
FROM ${BUILDER_IMAGE} AS build
WORKDIR /mcr/backend
COPY backend/ ./
ARG SERVICE
RUN case "${SERVICE}" in \
    api-gateway|auth-service|profile-service|friend-service|post-service|notification-service) ;; \
    *) echo 'Choose one supported MCR inherited SERVICE' >&2; exit 64 ;; \
    esac
ENV SQLX_OFFLINE=true CARGO_BUILD_JOBS=2
# Service build.rs uses vendored protoc and the current service's ../../proto.
# No database, provider secret or external service is needed during compilation.
RUN cargo +1.88.0 build --release --locked --package "${SERVICE}" --bin "${SERVICE}" \
    && install -D -m 0755 "target/release/${SERVICE}" /out/service

FROM ${RUNTIME_IMAGE} AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system mcr \
    && useradd --system --gid mcr --no-create-home --home-dir /nonexistent mcr
WORKDIR /app
ARG SERVICE
ARG SOURCE_REVISION=unrecorded
COPY --from=build /out/service /usr/local/bin/mcr-service
# Migrator::new needs this exact compiled CARGO_MANIFEST_DIR in the runtime image.
COPY --from=build /mcr/backend/services/${SERVICE}/migrations /mcr/backend/services/${SERVICE}/migrations
COPY --from=build /mcr/backend/services/${SERVICE}/runtime-grants.sql /mcr/backend/services/${SERVICE}/runtime-grants.sql
LABEL org.opencontainers.image.revision="${SOURCE_REVISION}" \
      org.mcr.service="${SERVICE}" \
      org.mcr.qualification="NOT_RUN"
USER mcr
# No env file, default database URL, exposed host port, startup seed or provider secret.
ENTRYPOINT ["/usr/local/bin/mcr-service"]
