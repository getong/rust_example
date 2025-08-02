#!/bin/bash

echo "Testing Stream Chat V8 Integration..."
echo "======================================"

# Test token generation
echo -e "\n1. Testing token generation for user 'john':"
curl -s "http://localhost:8080/stream-chat/token?data=john" | grep -A5 -B5 "Token:" || echo "Failed to get token"

# Test authentication endpoint
echo -e "\n\n2. Testing authentication endpoint:"
curl -s "http://localhost:8080/stream-chat/authenticate?data=jane" | head -20

# Test user context endpoint
echo -e "\n\n3. Testing user context endpoint:"
curl -s "http://localhost:8080/stream-chat/user-context?data=john" | head -20

# Test setup endpoint
echo -e "\n\n4. Testing setup endpoint:"
curl -s "http://localhost:8080/stream-chat/setup" | head -20