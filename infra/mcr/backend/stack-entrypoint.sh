#!/bin/sh
# Decode the private key supplied as base64 so the env file stays single-line.
set -eu
if [ -n "${AUTH_JWT_PRIVATE_KEY_PEM_B64:-}" ]; then
    AUTH_JWT_PRIVATE_KEY_PEM=$(printf '%s' "${AUTH_JWT_PRIVATE_KEY_PEM_B64}" | base64 -d)
    export AUTH_JWT_PRIVATE_KEY_PEM
fi
if [ "$#" -lt 1 ]; then
    echo "usage: stack-entrypoint SERVICE" >&2
    exit 64
fi
case "$1" in
    api-gateway|auth-service|profile-service|friend-service|post-service|notification-service|concert-service) ;;
    *) echo "unsupported service: $1" >&2; exit 64 ;;
esac
exec "/usr/local/bin/$1"
