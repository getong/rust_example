#!/bin/bash

set -e

echo "🚀 Starting Kafka and Zookeeper..."

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "❌ Docker is not running. Please start Docker and try again."
    exit 1
fi

# Start services
docker-compose up -d

echo "⏳ Waiting for Kafka to be ready..."
sleep 10

# Wait for Kafka to be healthy
MAX_RETRIES=30
RETRY_COUNT=0

while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    if docker exec kafka kafka-broker-api-versions --bootstrap-server localhost:9092 > /dev/null 2>&1; then
        echo "✅ Kafka is ready!"
        break
    fi
    RETRY_COUNT=$((RETRY_COUNT + 1))
    echo "⏳ Waiting for Kafka... ($RETRY_COUNT/$MAX_RETRIES)"
    sleep 2
done

if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
    echo "❌ Kafka failed to start after $MAX_RETRIES attempts"
    exit 1
fi

# Create the events topic
echo "📝 Creating 'events' topic..."
docker exec kafka kafka-topics --create \
    --bootstrap-server localhost:9092 \
    --topic events \
    --partitions 3 \
    --replication-factor 1 \
    --if-not-exists

echo "✅ Kafka setup complete!"
echo ""
echo "📊 Kafka is running on: localhost:9092"
echo "🔍 Zookeeper is running on: localhost:2181"
echo ""
echo "Useful commands:"
echo "  - List topics: docker exec kafka kafka-topics --list --bootstrap-server localhost:9092"
echo "  - Produce messages: docker exec -it kafka kafka-console-producer --bootstrap-server localhost:9092 --topic events"
echo "  - Consume messages: docker exec -it kafka kafka-console-consumer --bootstrap-server localhost:9092 --topic events --from-beginning"
echo "  - Stop Kafka: docker-compose down"
echo "  - View logs: docker-compose logs -f"
