#!/usr/bin/env bash
set -euo pipefail

KAFKA_CONTAINER_NAME="${KAFKA_CONTAINER_NAME:-confluent-kafka-example}"
if docker container inspect "$KAFKA_CONTAINER_NAME" >/dev/null 2>&1; then
    if [[ "$(docker inspect --format '{{index .Config.Labels "confluent-kafka-example"}}' "$KAFKA_CONTAINER_NAME")" != 'true' ]]; then
        echo "Container $KAFKA_CONTAINER_NAME is not managed by this example." >&2
        exit 1
    fi
    docker stop "$KAFKA_CONTAINER_NAME"
    docker rm "$KAFKA_CONTAINER_NAME"
fi
echo 'Kafka stopped. The Docker data volume is preserved.'
