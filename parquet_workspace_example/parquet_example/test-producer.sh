#!/bin/bash

# Script to send test events to Kafka

echo "📤 Sending test events to Kafka..."

# Send some test JSON events
docker exec -i kafka kafka-console-producer \
    --bootstrap-server localhost:9092 \
    --topic events << EOF
{"id": "event-001", "value": 42.5}
{"id": "event-002", "value": 123.45}
{"id": "event-003", "value": 99.99}
{"id": "event-004", "value": 55.0}
{"id": "event-005", "value": 200.25}
EOF

echo "✅ Test events sent successfully!"
echo ""
echo "To consume these events, run:"
echo "  docker exec -it kafka kafka-console-consumer --bootstrap-server localhost:9092 --topic events --from-beginning"
