#!/bin/bash

echo "=== Stream Chat V8 Integration Test Results ==="
echo "=============================================="
echo ""

# Test authentication
echo "1. Authentication Test (user: john):"
echo "------------------------------------"
curl -s "http://localhost:8080/stream-chat/authenticate?data=john" | \
  grep -o '{.*}' | jq '.' 2>/dev/null || echo "Failed to parse JSON"

echo -e "\n2. User Context Test (user: jane):"
echo "------------------------------------"
curl -s "http://localhost:8080/stream-chat/user-context?data=jane" | \
  grep -o '{.*}' | jq '.' 2>/dev/null || echo "Failed to parse JSON"

echo -e "\n3. Analytics Test:"
echo "------------------------------------"
curl -s "http://localhost:8080/stream-chat/analytics" | \
  grep -o '{.*}' | jq '.' 2>/dev/null || echo "Failed to parse JSON"

echo -e "\n4. Setup/Configuration Test:"
echo "------------------------------------"
curl -s "http://localhost:8080/stream-chat/setup" | \
  grep -o '{.*}' | jq '.' 2>/dev/null || echo "Failed to parse JSON"

echo -e "\n5. Token Generation Demo (via /stream-chat/token):"
echo "------------------------------------"
echo "Visit: http://localhost:8080/stream-chat/token?data=john"
echo "This shows a full HTML demo page with:"
echo "- Stream.io documentation pattern"
echo "- Rust implementation details"
echo "- Generated token display"
echo "- Links to test different users"