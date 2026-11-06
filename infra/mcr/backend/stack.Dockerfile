# syntax=docker/dockerfile:1
# BE44 stack image: builds all seven backend binaries once, then serves each
# compose service from the same image. Repository-root context:
#   docker build -f infra/mcr/backend/stack.Dockerfile .
# A local reproducible stack, not a production qualification.
ARG BUILDER_IMAGE=rust:1.88.0-bookworm
ARG RUNTIME_IMAGE=debian:bookworm-slim
FROM ${BUILDER_IMAGE} AS build
ARG CARGO_BUILD_JOBS=3
WORKDIR /mcr/backend
COPY backend/ ./
ENV SQLX_OFFLINE=true CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS}
RUN cargo +1.88.0 build --release --locked --bins \
      -p api-gateway -p auth-service -p profile-service -p friend-service \
      -p post-service -p notification-service -p concert-service \
    && mkdir -p /out \
    && for service in api-gateway auth-service profile-service friend-service \
         post-service notification-service concert-service; do \
         install -D -m 0755 "target/release/${service}" "/out/${service}"; \
       done

FROM ${RUNTIME_IMAGE} AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libgcc-s1 curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system mcr \
    && useradd --system --gid mcr --no-create-home --home-dir /nonexistent mcr
WORKDIR /app
ARG SOURCE_REVISION=unrecorded
COPY --from=build /out/ /usr/local/bin/
# Migrator::new and the Concert release check resolve the compiled
# CARGO_MANIFEST_DIR; keep every migration set at that exact path.
COPY --from=build /mcr/backend/services/ /mcr/backend/services/
COPY infra/mcr/backend/stack-entrypoint.sh /usr/local/bin/stack-entrypoint
COPY infra/mcr/backend/migrate-all.sh /usr/local/bin/migrate-all.sh
RUN chmod 0755 /usr/local/bin/stack-entrypoint /usr/local/bin/migrate-all.sh
LABEL org.opencontainers.image.revision="${SOURCE_REVISION}" \
      org.mcr.stack="seven-service-local" \
      org.mcr.qualification="NOT_RUN"
USER mcr
ENTRYPOINT ["/usr/local/bin/stack-entrypoint"]
