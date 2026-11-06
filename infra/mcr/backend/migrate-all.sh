#!/bin/sh
# Apply and then verify every owned migration set before any service starts.
set -eu
for service in api-gateway auth-service profile-service friend-service \
               post-service notification-service concert-service; do
    echo "migrate: $service"
    "/usr/local/bin/$service" --migrate
    "/usr/local/bin/$service" --verify-migrations
    echo "verified: $service"
done
