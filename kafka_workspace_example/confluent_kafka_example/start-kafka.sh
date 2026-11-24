#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
KAFKA_CONTAINER_NAME="${KAFKA_CONTAINER_NAME:-confluent-kafka-example}"
KAFKA_IMAGE="${KAFKA_IMAGE:-confluent-kafka-example:local}"
KAFKA_NETWORK="${KAFKA_NETWORK:-confluent-kafka-example}"
KAFKA_VOLUME="${KAFKA_VOLUME:-confluent-kafka-example-data}"
KAFKA_PORT="${KAFKA_PORT:-9092}"
KAFKA_TOPIC="${KAFKA_TOPIC:-confluent-rust-demo}"
KAFKA_PARTITIONS="${KAFKA_PARTITIONS:-3}"
KAFKA_ADVERTISED_HOST="${KAFKA_ADVERTISED_HOST:-localhost}"

command -v docker >/dev/null 2>&1 || { echo 'Please install Docker first.' >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo 'Please start Docker first.' >&2; exit 1; }

if docker container inspect "$KAFKA_CONTAINER_NAME" >/dev/null 2>&1; then
    if [[ "$(docker inspect --format '{{index .Config.Labels "confluent-kafka-example"}}' "$KAFKA_CONTAINER_NAME")" != 'true' ]]; then
        echo "Container $KAFKA_CONTAINER_NAME is not managed by this example." >&2
        exit 1
    fi
    echo "Reusing $KAFKA_CONTAINER_NAME; existing container configuration takes precedence."
    docker start "$KAFKA_CONTAINER_NAME" >/dev/null
else
    docker build --tag "$KAFKA_IMAGE" "$PROJECT_DIR"
    docker network inspect "$KAFKA_NETWORK" >/dev/null 2>&1 || docker network create "$KAFKA_NETWORK" >/dev/null
    docker volume inspect "$KAFKA_VOLUME" >/dev/null 2>&1 || docker volume create "$KAFKA_VOLUME" >/dev/null
    docker run --detach \
        --name "$KAFKA_CONTAINER_NAME" \
        --label confluent-kafka-example=true \
        --network "$KAFKA_NETWORK" \
        --network-alias confluent-kafka \
        --publish "127.0.0.1:${KAFKA_PORT}:9092" \
        --volume "${KAFKA_VOLUME}:/var/lib/kafka/data" \
        --env "KAFKA_ADVERTISED_LISTENERS=INTERNAL://confluent-kafka:29092,EXTERNAL://${KAFKA_ADVERTISED_HOST}:${KAFKA_PORT}" \
        "$KAFKA_IMAGE" >/dev/null
fi

echo 'Waiting for Kafka (up to 180 seconds)...'
deadline=$((SECONDS + 180))
until docker exec "$KAFKA_CONTAINER_NAME" kafka-topics --bootstrap-server localhost:29092 --list >/dev/null 2>&1; do
    if [[ "$(docker inspect --format '{{.State.Running}}' "$KAFKA_CONTAINER_NAME")" != 'true' ]] || (( SECONDS >= deadline )); then
        docker logs --tail 80 "$KAFKA_CONTAINER_NAME" >&2
        echo 'Kafka failed to become ready.' >&2
        exit 1
    fi
    sleep 2
done

docker exec "$KAFKA_CONTAINER_NAME" kafka-topics \
    --bootstrap-server localhost:29092 \
    --create --if-not-exists --topic "$KAFKA_TOPIC" \
    --partitions "$KAFKA_PARTITIONS" --replication-factor 1
docker exec "$KAFKA_CONTAINER_NAME" kafka-topics \
    --bootstrap-server localhost:29092 --describe --topic "$KAFKA_TOPIC"
echo "Kafka ready: ${KAFKA_ADVERTISED_HOST}:${KAFKA_PORT}, topic=$KAFKA_TOPIC"
